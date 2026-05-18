use std::future::Future;
use std::time::Duration;

use tracing::{debug, info, warn};

use crate::errors::{is_529_error, ApiErrorType, QueryError};

/// Default maximum number of retries.
pub const DEFAULT_MAX_RETRIES: usize = 10;

/// Maximum consecutive 529 errors before giving up.
pub const MAX_529_RETRIES: usize = 3;

/// Base delay for exponential backoff (ms).
pub const BASE_DELAY_MS: u64 = 500;

/// Maximum backoff delay (ms).
pub const MAX_BACKOFF_MS: u64 = 30_000;

/// Context passed to retryable operations for adaptive behavior.
#[derive(Debug, Clone)]
pub struct RetryContext {
    pub model: String,
    pub max_tokens_override: Option<u64>,
}

/// Configuration for the retry loop.
#[derive(Debug, Clone)]
pub struct RetryOptions {
    pub max_retries: usize,
    pub model: String,
    pub fallback_model: Option<String>,
    /// Pre-seeded 529 counter (for non-streaming fallback after streaming 529).
    pub initial_consecutive_529: usize,
}

impl Default for RetryOptions {
    fn default() -> Self {
        Self {
            max_retries: DEFAULT_MAX_RETRIES,
            model: String::new(),
            fallback_model: None,
            initial_consecutive_529: 0,
        }
    }
}

/// Result of a retry attempt — either continue or yield an error to the caller.
#[derive(Debug)]
pub enum RetryDecision {
    /// Retry after the given duration.
    Retry { delay: Duration },
    /// Stop retrying and return the error.
    Abort,
    /// Switch to fallback model.
    Fallback { model: String },
}

/// Determine retry behavior based on the error.
pub fn decide_retry(
    error: &QueryError,
    attempt: usize,
    consecutive_529: usize,
    options: &RetryOptions,
) -> RetryDecision {
    // Check if we've exhausted retries
    if attempt >= options.max_retries {
        return RetryDecision::Abort;
    }

    // Check for 529 overload — limited consecutive retries
    if is_529_error(error) {
        if consecutive_529 >= MAX_529_RETRIES {
            // Try fallback model if available
            if let Some(ref fallback) = options.fallback_model {
                return RetryDecision::Fallback {
                    model: fallback.clone(),
                };
            }
            return RetryDecision::Abort;
        }
    }

    // Check if error is retryable at all
    if !is_error_retryable(error) {
        return RetryDecision::Abort;
    }

    // Calculate backoff delay
    let delay = calculate_backoff(attempt, error);
    RetryDecision::Retry { delay }
}

/// Check if an error type should be retried.
pub fn is_error_retryable(error: &QueryError) -> bool {
    match error {
        QueryError::Api(api_err) => api_err.error_type.is_retryable(),
        QueryError::Connection { .. } => true,
        QueryError::Timeout => true,
        QueryError::Aborted => false,
        QueryError::CannotRetry(_) => false,
        QueryError::FallbackTriggered { .. } => false,
        QueryError::StreamParse(_) => false,
        QueryError::Internal(_) => false,
    }
}

/// Calculate exponential backoff with jitter.
pub fn calculate_backoff(attempt: usize, error: &QueryError) -> Duration {
    // Use retry-after from rate limit if available
    if let QueryError::Api(api_err) = error {
        if let ApiErrorType::RateLimit {
            retry_after_ms: Some(ms),
            ..
        } = &api_err.error_type
        {
            return Duration::from_millis(*ms);
        }
    }

    // Exponential backoff: BASE_DELAY * 2^(attempt-1), capped at MAX_BACKOFF
    let base = BASE_DELAY_MS;
    let exponential = base.saturating_mul(1u64 << (attempt.min(63) as u64));
    let capped = exponential.min(MAX_BACKOFF_MS);

    // Add jitter: random value between 0 and capped/4
    let jitter = capped / 4;
    let jittered = if jitter > 0 {
        use std::time::SystemTime;
        let seed = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .map(|d| d.as_millis() as u64)
            .unwrap_or(0);
        capped + (seed % jitter)
    } else {
        capped
    };

    Duration::from_millis(jittered)
}

/// Execute an operation with retry logic.
///
/// Returns the result on success, or the last error after exhausting retries.
/// Yields intermediate error events via the callback for progress reporting.
pub async fn with_retry<T, F, Fut>(
    options: RetryOptions,
    mut operation: F,
    mut on_error: impl FnMut(&QueryError, usize),
) -> Result<T, QueryError>
where
    F: FnMut(&RetryContext) -> Fut,
    Fut: Future<Output = Result<T, QueryError>>,
{
    let retry_context = RetryContext {
        model: options.model.clone(),
        max_tokens_override: None,
    };

    let mut consecutive_529 = options.initial_consecutive_529;
    let mut current_model = options.model.clone();

    for attempt in 1..=options.max_retries + 1 {
        let ctx = RetryContext {
            model: current_model.clone(),
            max_tokens_override: retry_context.max_tokens_override,
        };

        match operation(&ctx).await {
            Ok(result) => return Ok(result),
            Err(error) => {
                on_error(&error, attempt);

                // Track 529 count
                if is_529_error(&error) {
                    consecutive_529 += 1;
                } else {
                    consecutive_529 = 0;
                }

                let decision = decide_retry(&error, attempt, consecutive_529, &options);

                match decision {
                    RetryDecision::Retry { delay } => {
                        warn!(
                            attempt,
                            delay_ms = delay.as_millis(),
                            error = %error,
                            "Retrying API request"
                        );
                        tokio::time::sleep(delay).await;
                    }
                    RetryDecision::Fallback { model } => {
                        info!(
                            from = current_model,
                            to = model,
                            "Falling back to alternative model"
                        );
                        current_model = model;
                        consecutive_529 = 0;
                        // Retry immediately with fallback
                    }
                    RetryDecision::Abort => {
                        debug!(attempt, error = %error, "Exhausted retries");
                        return Err(error);
                    }
                }
            }
        }
    }

    unreachable!("Loop should return via Abort or success")
}

/// Execute with retry and return both the result and the number of attempts.
pub async fn with_retry_tracked<T, F, Fut>(
    options: RetryOptions,
    mut operation: F,
    mut on_error: impl FnMut(&QueryError, usize),
) -> Result<(T, usize), QueryError>
where
    F: FnMut(&RetryContext) -> Fut,
    Fut: Future<Output = Result<T, QueryError>>,
{
    let mut consecutive_529 = options.initial_consecutive_529;
    let mut current_model = options.model.clone();

    for attempt in 1..=options.max_retries + 1 {
        let ctx = RetryContext {
            model: current_model.clone(),
            max_tokens_override: None,
        };

        match operation(&ctx).await {
            Ok(result) => return Ok((result, attempt)),
            Err(error) => {
                on_error(&error, attempt);

                if is_529_error(&error) {
                    consecutive_529 += 1;
                } else {
                    consecutive_529 = 0;
                }

                let decision = decide_retry(&error, attempt, consecutive_529, &options);

                match decision {
                    RetryDecision::Retry { delay } => {
                        warn!(
                            attempt,
                            delay_ms = delay.as_millis(),
                            error = %error,
                            "Retrying API request"
                        );
                        tokio::time::sleep(delay).await;
                    }
                    RetryDecision::Fallback { model } => {
                        info!(
                            from = current_model,
                            to = model,
                            "Falling back to alternative model"
                        );
                        current_model = model;
                        consecutive_529 = 0;
                    }
                    RetryDecision::Abort => {
                        debug!(attempt, error = %error, "Exhausted retries");
                        return Err(error);
                    }
                }
            }
        }
    }

    unreachable!("Loop should return via Abort or success")
}
