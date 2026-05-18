use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

use cc_query::api_client::{ApiClient, ApiConfig};
use cc_query::api_types::{MessageRequest, MessageResponse, StreamEvent};
use cc_query::errors::QueryError;
use cc_query::retry::RetryOptions;
use futures::Stream;
use futures::StreamExt;
use tokio::sync::RwLock;
use tracing::{debug, info, warn};

/// Rate limit configuration.
#[derive(Debug, Clone)]
pub struct RateLimitConfig {
    /// Maximum requests per minute.
    pub max_requests_per_minute: usize,
    /// Maximum tokens per minute.
    pub max_tokens_per_minute: usize,
    /// Maximum concurrent requests.
    pub max_concurrent: usize,
}

impl Default for RateLimitConfig {
    fn default() -> Self {
        Self {
            max_requests_per_minute: 50,
            max_tokens_per_minute: 100_000,
            max_concurrent: 5,
        }
    }
}

/// Cache entry for API responses.
#[derive(Debug, Clone)]
struct CacheEntry {
    response: MessageResponse,
    timestamp: Instant,
    ttl: Duration,
}

impl CacheEntry {
    fn is_expired(&self) -> bool {
        self.timestamp.elapsed() > self.ttl
    }
}

/// Request tracking information.
#[derive(Debug, Clone, Default)]
pub struct RequestStats {
    pub total_requests: usize,
    pub total_retries: usize,
    pub total_errors: usize,
    pub total_cache_hits: usize,
    pub total_tokens_used: u64,
    pub last_request_time: Option<Instant>,
    pub rate_limit_hits: usize,
}

/// Request cache for deduplicating identical requests.
pub struct RequestCache {
    entries: RwLock<HashMap<String, CacheEntry>>,
    ttl: Duration,
    max_size: usize,
}

impl RequestCache {
    pub fn new(ttl: Duration, max_size: usize) -> Self {
        Self {
            entries: RwLock::new(HashMap::new()),
            ttl,
            max_size,
        }
    }

    pub async fn get(&self, key: &str) -> Option<MessageResponse> {
        let entries = self.entries.read().await;
        entries.get(key).and_then(|entry| {
            if entry.is_expired() {
                None
            } else {
                Some(entry.response.clone())
            }
        })
    }

    pub async fn insert(&self, key: String, response: MessageResponse) {
        let mut entries = self.entries.write().await;

        // Evict if at capacity
        if entries.len() >= self.max_size {
            // Remove oldest entries
            let mut oldest_key = None;
            let mut oldest_time = Instant::now();
            for (k, v) in entries.iter() {
                if v.timestamp < oldest_time {
                    oldest_time = v.timestamp;
                    oldest_key = Some(k.clone());
                }
            }
            if let Some(key) = oldest_key {
                entries.remove(&key);
            }
        }

        entries.insert(
            key,
            CacheEntry {
                response,
                timestamp: Instant::now(),
                ttl: self.ttl,
            },
        );
    }

    pub async fn clear(&self) {
        self.entries.write().await.clear();
    }

    pub async fn len(&self) -> usize {
        self.entries.read().await.len()
    }
}

/// Rate limiter for API requests.
pub struct RateLimiter {
    config: RateLimitConfig,
    request_times: RwLock<Vec<Instant>>,
    concurrent_count: RwLock<usize>,
}

impl RateLimiter {
    pub fn new(config: RateLimitConfig) -> Self {
        Self {
            config,
            request_times: RwLock::new(Vec::new()),
            concurrent_count: RwLock::new(0),
        }
    }

    /// Wait until a request slot is available.
    pub async fn wait_for_slot(&self) {
        loop {
            let now = Instant::now();
            let window_start = now - Duration::from_secs(60);

            {
                let mut times = self.request_times.write().await;
                // Remove requests outside the window
                times.retain(|t| *t > window_start);

                if times.len() < self.config.max_requests_per_minute {
                    times.push(now);
                    return;
                }
            }

            // Wait a bit before retrying
            tokio::time::sleep(Duration::from_millis(100)).await;
        }
    }

    /// Acquire a concurrent request slot.
    pub async fn acquire_concurrent(&self) {
        loop {
            let mut count = self.concurrent_count.write().await;
            if *count < self.config.max_concurrent {
                *count += 1;
                return;
            }
            drop(count);
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// Release a concurrent request slot.
    pub async fn release_concurrent(&self) {
        let mut count = self.concurrent_count.write().await;
        *count = count.saturating_sub(1);
    }

    /// Record token usage.
    pub fn record_tokens(&self, _tokens: u64) {
        // Could implement per-minute token tracking here
    }
}

/// API service — wraps the query client with rate limiting, caching, and retry orchestration.
pub struct ApiService {
    pub client: Arc<ApiClient>,
    pub cache: Arc<RequestCache>,
    pub rate_limiter: Arc<RateLimiter>,
    pub stats: Arc<RwLock<RequestStats>>,
}

impl ApiService {
    pub fn new(config: ApiConfig, rate_limit_config: RateLimitConfig) -> Result<Self, QueryError> {
        let client = Arc::new(ApiClient::new(config)?);
        Ok(Self {
            client,
            cache: Arc::new(RequestCache::new(Duration::from_secs(300), 100)),
            rate_limiter: Arc::new(RateLimiter::new(rate_limit_config)),
            stats: Arc::new(RwLock::new(RequestStats::default())),
        })
    }

    /// Generate a cache key from a request.
    fn cache_key(request: &MessageRequest) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        request.model.hash(&mut hasher);
        request.max_tokens.hash(&mut hasher);
        serde_json::to_string(&request.messages)
            .unwrap_or_default()
            .hash(&mut hasher);
        if let Some(ref tools) = request.tools {
            serde_json::to_string(tools)
                .unwrap_or_default()
                .hash(&mut hasher);
        }
        if let Some(ref system) = request.system {
            serde_json::to_string(system)
                .unwrap_or_default()
                .hash(&mut hasher);
        }
        format!("{:x}", hasher.finish())
    }

    /// Send a non-streaming message with caching and rate limiting.
    pub async fn send_message(&self, request: MessageRequest) -> Result<MessageResponse, QueryError> {
        // Check cache first
        let key = Self::cache_key(&request);
        if let Some(cached) = self.cache.get(&key).await {
            let mut stats = self.stats.write().await;
            stats.total_cache_hits += 1;
            stats.total_requests += 1;
            debug!(cache_key = %key, "Cache hit");
            return Ok(cached);
        }

        // Wait for rate limit slot
        self.rate_limiter.wait_for_slot().await;
        self.rate_limiter.acquire_concurrent().await;

        let result = self.client.send_message(&request).await;

        self.rate_limiter.release_concurrent().await;

        match &result {
            Ok(response) => {
                // Cache the response
                self.cache.insert(key, response.clone()).await;

                // Update stats
                let mut stats = self.stats.write().await;
                stats.total_requests += 1;
                stats.total_tokens_used += response.usage.input_tokens + response.usage.output_tokens;
                stats.last_request_time = Some(Instant::now());
            }
            Err(e) => {
                let mut stats = self.stats.write().await;
                stats.total_requests += 1;
                stats.total_errors += 1;
                warn!(error = %e, "API request failed");
            }
        }

        result
    }

    /// Send a streaming message with rate limiting and retry orchestration.
    pub fn stream_message(
        &self,
        request: MessageRequest,
        retry_options: RetryOptions,
    ) -> impl Stream<Item = Result<StreamEvent, QueryError>> + Send + 'static {
        let rate_limiter = self.rate_limiter.clone();
        let stats = self.stats.clone();
        let client = Arc::clone(&self.client);

        let stream = async_stream::stream! {
            // Wait for rate limit slot
            rate_limiter.wait_for_slot().await;
            rate_limiter.acquire_concurrent().await;

            let mut event_stream = client.stream_message_with_retry(request, retry_options);

            while let Some(event_result) = event_stream.next().await {
                match &event_result {
                    Ok(event) => {
                        // Count tokens from message_delta events
                        if let StreamEvent::MessageDelta { usage, .. } = event {
                            let mut stats_guard = stats.write().await;
                            stats_guard.total_tokens_used += usage.output_tokens;
                            stats_guard.last_request_time = Some(Instant::now());
                        }
                    }
                    Err(e) => {
                        let mut stats_guard = stats.write().await;
                        stats_guard.total_errors += 1;
                        warn!(error = %e, "Stream error");
                    }
                }
                yield event_result;
            }

            rate_limiter.release_concurrent().await;
        };

        Box::pin(stream)
    }

    /// Get current request statistics.
    pub async fn get_stats(&self) -> RequestStats {
        self.stats.read().await.clone()
    }

    /// Clear the request cache.
    pub async fn clear_cache(&self) {
        self.cache.clear().await;
    }

    /// Get cache size.
    pub async fn cache_size(&self) -> usize {
        self.cache.len().await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_cache_entry_expiry() {
        let entry = CacheEntry {
            response: cc_query::api_types::MessageResponse {
                id: "test".to_string(),
                response_type: "message".to_string(),
                role: "assistant".to_string(),
                model: "test".to_string(),
                content: vec![],
                usage: cc_query::api_types::Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_input_tokens: None,
                    cache_creation_input_tokens: None,
                },
                stop_reason: None,
                stop_sequence: None,
            },
            timestamp: Instant::now() - Duration::from_secs(600),
            ttl: Duration::from_secs(300),
        };
        assert!(entry.is_expired());
    }

    #[test]
    fn test_cache_entry_not_expired() {
        let entry = CacheEntry {
            response: cc_query::api_types::MessageResponse {
                id: "test".to_string(),
                response_type: "message".to_string(),
                role: "assistant".to_string(),
                model: "test".to_string(),
                content: vec![],
                usage: cc_query::api_types::Usage {
                    input_tokens: 0,
                    output_tokens: 0,
                    cache_read_input_tokens: None,
                    cache_creation_input_tokens: None,
                },
                stop_reason: None,
                stop_sequence: None,
            },
            timestamp: Instant::now(),
            ttl: Duration::from_secs(300),
        };
        assert!(!entry.is_expired());
    }

    #[test]
    fn test_rate_limit_config_defaults() {
        let config = RateLimitConfig::default();
        assert_eq!(config.max_requests_per_minute, 50);
        assert_eq!(config.max_tokens_per_minute, 100_000);
        assert_eq!(config.max_concurrent, 5);
    }

    #[test]
    fn test_request_stats_default() {
        let stats = RequestStats::default();
        assert_eq!(stats.total_requests, 0);
        assert_eq!(stats.total_retries, 0);
        assert_eq!(stats.total_errors, 0);
        assert_eq!(stats.total_cache_hits, 0);
        assert_eq!(stats.total_tokens_used, 0);
        assert!(stats.last_request_time.is_none());
        assert_eq!(stats.rate_limit_hits, 0);
    }

    #[tokio::test]
    async fn test_request_cache_insert_and_get() {
        let cache = RequestCache::new(Duration::from_secs(300), 100);
        let response = cc_query::api_types::MessageResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            model: "test".to_string(),
            content: vec![],
            usage: cc_query::api_types::Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            },
            stop_reason: None,
            stop_sequence: None,
        };
        cache.insert("key1".to_string(), response.clone()).await;
        let retrieved = cache.get("key1").await;
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().id, "test");
    }

    #[tokio::test]
    async fn test_request_cache_miss() {
        let cache = RequestCache::new(Duration::from_secs(300), 100);
        let result = cache.get("nonexistent").await;
        assert!(result.is_none());
    }

    #[tokio::test]
    async fn test_request_cache_clear() {
        let cache = RequestCache::new(Duration::from_secs(300), 100);
        let response = cc_query::api_types::MessageResponse {
            id: "test".to_string(),
            response_type: "message".to_string(),
            role: "assistant".to_string(),
            model: "test".to_string(),
            content: vec![],
            usage: cc_query::api_types::Usage {
                input_tokens: 0,
                output_tokens: 0,
                cache_read_input_tokens: None,
                cache_creation_input_tokens: None,
            },
            stop_reason: None,
            stop_sequence: None,
        };
        cache.insert("key1".to_string(), response).await;
        assert_eq!(cache.len().await, 1);
        cache.clear().await;
        assert_eq!(cache.len().await, 0);
    }
}
