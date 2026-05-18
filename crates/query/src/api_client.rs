use std::time::Duration;

use async_stream::stream;
use futures::Stream;
use futures::StreamExt;
use reqwest::header::{HeaderMap, HeaderValue};
use tracing::{debug, info, trace, warn};

use crate::api_types::{
    MessageRequest, MessageResponse, StreamEvent, StreamingMessage, ToolDefinition,
};
use crate::errors::{ApiError, QueryError};
use crate::retry::RetryOptions;

/// Beta headers for the Anthropic API.
pub const PROMPT_CACHING_BETA: &str = "prompt-caching-2024-07-31";
const API_VERSION: &str = "2023-06-01";

/// Configuration for the API client.
#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub api_key: String,
    pub base_url: String,
    pub timeout: Duration,
    pub anthropic_version: String,
    pub betas: Vec<String>,
}

impl Default for ApiConfig {
    fn default() -> Self {
        Self {
            api_key: String::new(),
            base_url: "https://api.anthropic.com".to_string(),
            timeout: Duration::from_secs(600),
            anthropic_version: API_VERSION.to_string(),
            betas: vec![PROMPT_CACHING_BETA.to_string()],
        }
    }
}

/// Anthropic API client with streaming support.
pub struct ApiClient {
    client: reqwest::Client,
    config: ApiConfig,
}

impl ApiClient {
    pub fn new(config: ApiConfig) -> Result<Self, QueryError> {
        let client = reqwest::Client::builder()
            .timeout(config.timeout)
            .build()
            .map_err(|e| QueryError::Connection {
                message: format!("Failed to create HTTP client: {e}"),
            })?;

        Ok(Self { client, config })
    }

    fn build_headers(&self) -> HeaderMap {
        let mut headers = HeaderMap::new();

        headers.insert(
            "x-api-key",
            HeaderValue::from_str(&self.config.api_key)
                .unwrap_or_else(|_| HeaderValue::from_static("")),
        );
        headers.insert(
            "anthropic-version",
            HeaderValue::from_str(&self.config.anthropic_version)
                .unwrap_or_else(|_| HeaderValue::from_static(API_VERSION)),
        );
        headers.insert(
            "content-type",
            HeaderValue::from_static("application/json"),
        );

        if !self.config.betas.is_empty() {
            let betas = self.config.betas.join(", ");
            headers.insert(
                "anthropic-beta",
                HeaderValue::from_str(&betas)
                    .unwrap_or_else(|_| HeaderValue::from_static("")),
            );
        }

        headers
    }

    fn messages_url(&self) -> String {
        format!("{}/v1/messages", self.config.base_url)
    }

    /// Send a non-streaming message request.
    pub async fn send_message(
        &self,
        request: &MessageRequest,
    ) -> Result<MessageResponse, QueryError> {
        let headers = self.build_headers();
        let url = self.messages_url();

        let response = self
            .client
            .post(&url)
            .headers(headers.clone())
            .json(request)
            .send()
            .await
            .map_err(|e| {
                if e.is_timeout() {
                    QueryError::Timeout
                } else {
                    QueryError::Connection {
                        message: e.to_string(),
                    }
                }
            })?;

        let status = response.status().as_u16();
        let request_id = response
            .headers()
            .get("request-id")
            .and_then(|v| v.to_str().ok())
            .map(String::from);

        let body = response.text().await.map_err(|e| {
            QueryError::StreamParse(format!("Failed to read response body: {e}"))
        })?;

        if status >= 400 {
            let error_type = ApiError::classify(status, &body);
            let mut api_error = ApiError::new(error_type).with_status(status);
            if let Some(rid) = request_id {
                api_error = api_error.with_request_id(rid);
            }
            return Err(QueryError::Api(api_error));
        }

        let message_response: MessageResponse = serde_json::from_str(&body).map_err(|e| {
            QueryError::StreamParse(format!("Failed to parse response: {e}\nBody: {body}"))
        })?;

        debug!(
            model = message_response.model,
            input_tokens = message_response.usage.input_tokens,
            output_tokens = message_response.usage.output_tokens,
            "API request successful"
        );

        Ok(message_response)
    }

    /// Send a streaming message request, yielding StreamEvents.
    pub fn stream_message(
        &self,
        request: MessageRequest,
    ) -> impl Stream<Item = Result<StreamEvent, QueryError>> + Send + 'static {
        let client = self.client.clone();
        let headers = self.build_headers();
        let url = self.messages_url();

        let mut streaming_request = request;
        streaming_request.stream = Some(true);

        let body = match serde_json::to_string(&streaming_request) {
            Ok(b) => b,
            Err(e) => {
                return futures::stream::once(async move {
                    Err(QueryError::StreamParse(format!(
                        "Failed to serialize request: {e}"
                    )))
                })
                .boxed();
            }
        };

        let s = stream! {
            let response = match client
                .post(&url)
                .headers(headers)
                .body(body)
                .send()
                .await
            {
                Ok(r) => r,
                Err(e) => {
                    let err = if e.is_timeout() {
                        QueryError::Timeout
                    } else {
                        QueryError::Connection { message: e.to_string() }
                    };
                    yield Err(err);
                    return;
                }
            };

            let status = response.status().as_u16();
            let request_id = response
                .headers()
                .get("request-id")
                .and_then(|v| v.to_str().ok())
                .map(String::from);

            if status >= 400 {
                let body = response.text().await.unwrap_or_default();
                let error_type = ApiError::classify(status, &body);
                let mut api_error = ApiError::new(error_type).with_status(status);
                if let Some(rid) = request_id {
                    api_error = api_error.with_request_id(rid);
                }
                yield Err(QueryError::Api(api_error));
                return;
            }

            let mut byte_stream = response.bytes_stream();
            let mut event_type = String::new();
            let mut event_data = String::new();

            while let Some(chunk_result) = byte_stream.next().await {
                let chunk = match chunk_result {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(QueryError::StreamParse(format!("Stream error: {e}")));
                        return;
                    }
                };

                trace!(bytes = chunk.len(), "Received stream chunk");

                let text = String::from_utf8_lossy(&chunk);
                for line in text.lines() {
                    let line = line.trim();

                    if line.is_empty() {
                        if !event_type.is_empty() && !event_data.is_empty() {
                            if let Some(event_result) = parse_sse_event(&event_type, &event_data) {
                                yield event_result;
                            }
                        }
                        event_type.clear();
                        event_data.clear();
                        continue;
                    }

                    if let Some(data) = line.strip_prefix("data: ") {
                        event_data.push_str(data);
                    } else if let Some(event) = line.strip_prefix("event: ") {
                        event_type = event.to_string();
                    }
                }
            }

            if !event_type.is_empty() && !event_data.is_empty() {
                if let Some(event_result) = parse_sse_event(&event_type, &event_data) {
                    yield event_result;
                }
            }
        };

        s.boxed()
    }

    /// Send a streaming message with automatic retry on transient errors.
    pub fn stream_message_with_retry(
        &self,
        request: MessageRequest,
        retry_options: RetryOptions,
    ) -> impl Stream<Item = Result<StreamEvent, QueryError>> + Send + 'static {
        let config = self.config.clone();

        let s = stream! {
            let mut consecutive_529 = retry_options.initial_consecutive_529;
            let mut current_model = retry_options.model.clone();
            let mut current_request = request.clone();
            current_request.model = current_model.clone();

            for attempt in 1..=retry_options.max_retries + 1 {
                let api_client = match ApiClient::new(config.clone()) {
                    Ok(c) => c,
                    Err(e) => {
                        yield Err(e);
                        return;
                    }
                };

                let mut event_stream = api_client.stream_message(current_request.clone());
                let mut stream_had_error = false;

                while let Some(event_result) = event_stream.next().await {
                    match event_result {
                        Ok(event) => {
                            yield Ok(event);
                        }
                        Err(e) => {
                            if crate::errors::is_529_error(&e) {
                                consecutive_529 += 1;
                            } else {
                                consecutive_529 = 0;
                            }

                            if attempt < retry_options.max_retries
                                && crate::retry::is_error_retryable(&e)
                            {
                                if consecutive_529 >= crate::retry::MAX_529_RETRIES {
                                    if let Some(ref fallback) = retry_options.fallback_model {
                                        info!(from = current_model, to = fallback, "Fallback after 529");
                                        current_model = fallback.clone();
                                        current_request.model = current_model.clone();
                                        consecutive_529 = 0;
                                        stream_had_error = true;
                                        break;
                                    }
                                }

                                let delay = crate::retry::calculate_backoff(attempt, &e);
                                warn!(attempt, delay_ms = delay.as_millis(), error = %e, "Retrying stream");
                                tokio::time::sleep(delay).await;
                                stream_had_error = true;
                                break;
                            }

                            yield Err(e);
                            return;
                        }
                    }
                }

                if !stream_had_error {
                    return;
                }
            }
        };

        s.boxed()
    }

    pub fn accumulator_to_content(
        accumulator: &StreamingMessage,
    ) -> Vec<cc_core::messages::ContentBlockParam> {
        accumulator.to_content_blocks()
    }
}

fn parse_sse_event(event_type: &str, data: &str) -> Option<Result<StreamEvent, QueryError>> {
    match event_type {
        "message_start"
        | "content_block_start"
        | "content_block_delta"
        | "content_block_stop"
        | "message_delta"
        | "message_stop"
        | "ping" => match serde_json::from_str::<StreamEvent>(data) {
            Ok(event) => Some(Ok(event)),
            Err(e) => Some(Err(QueryError::StreamParse(
                format!("Failed to parse {event_type} event: {e}\nData: {data}")
            ))),
        },
        _ => match serde_json::from_str::<StreamEvent>(data) {
            Ok(event) => Some(Ok(event)),
            Err(_) => None,
        },
    }
}

/// Build a MessageRequest from conversation messages and tool definitions.
pub fn build_request(
    model: &str,
    max_tokens: u64,
    messages: Vec<crate::api_types::MessageParam>,
    system: Option<Vec<crate::api_types::SystemPromptBlock>>,
    tools: Option<Vec<ToolDefinition>>,
    tool_choice: Option<crate::api_types::ToolChoice>,
    temperature: Option<f64>,
    thinking: Option<crate::api_types::ThinkingConfig>,
) -> MessageRequest {
    MessageRequest {
        model: model.to_string(),
        max_tokens,
        messages,
        system,
        tools,
        tool_choice,
        temperature,
        top_p: None,
        top_k: None,
        thinking,
        metadata: None,
        stop_sequences: None,
        stream: None,
        extra: None,
    }
}
