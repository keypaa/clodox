use std::fmt;

use thiserror::Error;

/// Categorized API error types matching Anthropic's error taxonomy.
#[derive(Debug, Clone)]
pub enum ApiErrorType {
    RateLimit {
        retry_after_ms: Option<u64>,
        message: String,
    },
    Overloaded {
        message: String,
    },
    InvalidRequest {
        message: String,
    },
    Authentication {
        message: String,
    },
    Permission {
        message: String,
    },
    NotFound {
        message: String,
    },
    InternalServerError {
        message: String,
    },
    ServiceUnavailable {
        message: String,
    },
    Timeout {
        message: String,
    },
    ConnectionError {
        message: String,
        code: Option<String>,
    },
    PromptTooLong {
        actual_tokens: Option<u64>,
        limit_tokens: Option<u64>,
        message: String,
    },
    CreditBalanceTooLow {
        message: String,
    },
    OrganizationDisabled {
        message: String,
    },
    TokenRevoked {
        message: String,
    },
    CustomOffSwitch {
        message: String,
    },
    Unknown {
        status: Option<u16>,
        message: String,
    },
}

impl ApiErrorType {
    /// Whether this error is transient and should be retried.
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ApiErrorType::RateLimit { .. }
                | ApiErrorType::Overloaded { .. }
                | ApiErrorType::Timeout { .. }
                | ApiErrorType::ConnectionError { .. }
                | ApiErrorType::ServiceUnavailable { .. }
                | ApiErrorType::InternalServerError { .. }
        )
    }

    /// Whether this error indicates the model should be fallen back.
    pub fn should_fallback(&self) -> bool {
        matches!(
            self,
            ApiErrorType::Overloaded { .. }
                | ApiErrorType::InternalServerError { .. }
                | ApiErrorType::ServiceUnavailable { .. }
        )
    }
}

impl fmt::Display for ApiErrorType {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ApiErrorType::RateLimit { message, .. } => {
                write!(f, "Rate limit exceeded: {message}")
            }
            ApiErrorType::Overloaded { message } => {
                write!(f, "Server overloaded: {message}")
            }
            ApiErrorType::InvalidRequest { message } => {
                write!(f, "Invalid request: {message}")
            }
            ApiErrorType::Authentication { message } => {
                write!(f, "Authentication failed: {message}")
            }
            ApiErrorType::Permission { message } => {
                write!(f, "Permission denied: {message}")
            }
            ApiErrorType::NotFound { message } => {
                write!(f, "Not found: {message}")
            }
            ApiErrorType::InternalServerError { message } => {
                write!(f, "Internal server error: {message}")
            }
            ApiErrorType::ServiceUnavailable { message } => {
                write!(f, "Service unavailable: {message}")
            }
            ApiErrorType::Timeout { message } => {
                write!(f, "Request timed out: {message}")
            }
            ApiErrorType::ConnectionError { message, .. } => {
                write!(f, "Connection error: {message}")
            }
            ApiErrorType::PromptTooLong { message, .. } => {
                write!(f, "{message}")
            }
            ApiErrorType::CreditBalanceTooLow { message } => {
                write!(f, "{message}")
            }
            ApiErrorType::OrganizationDisabled { message } => {
                write!(f, "{message}")
            }
            ApiErrorType::TokenRevoked { message } => {
                write!(f, "{message}")
            }
            ApiErrorType::CustomOffSwitch { message } => {
                write!(f, "{message}")
            }
            ApiErrorType::Unknown { message, status } => {
                if let Some(s) = status {
                    write!(f, "API Error ({s}): {message}")
                } else {
                    write!(f, "API Error: {message}")
                }
            }
        }
    }
}

/// Main error type for the query crate.
#[derive(Debug, Error)]
pub enum QueryError {
    #[error("API error: {0}")]
    Api(#[from] ApiError),

    #[error("Connection error: {message}")]
    Connection { message: String },

    #[error("Request timed out")]
    Timeout,

    #[error("Abort requested")]
    Aborted,

    #[error("Cannot retry: {0}")]
    CannotRetry(#[source] Box<dyn std::error::Error + Send + Sync>),

    #[error("Model fallback triggered: {original} -> {fallback}")]
    FallbackTriggered { original: String, fallback: String },

    #[error("Stream parsing error: {0}")]
    StreamParse(String),

    #[error("Internal error: {0}")]
    Internal(#[from] anyhow::Error),
}

/// Wraps an HTTP response error with classification.
#[derive(Debug)]
pub struct ApiError {
    pub error_type: ApiErrorType,
    pub status: Option<u16>,
    pub request_id: Option<String>,
    pub raw_response: Option<String>,
}

impl ApiError {
    pub fn new(error_type: ApiErrorType) -> Self {
        Self {
            error_type,
            status: None,
            request_id: None,
            raw_response: None,
        }
    }

    pub fn with_status(mut self, status: u16) -> Self {
        self.status = Some(status);
        self
    }

    pub fn with_request_id(mut self, request_id: String) -> Self {
        self.request_id = Some(request_id);
        self
    }

    pub fn with_raw_response(mut self, raw: String) -> Self {
        self.raw_response = Some(raw);
        self
    }

    /// Classify an HTTP status code and error body into an ApiErrorType.
    pub fn classify(status: u16, body: &str) -> ApiErrorType {
        match status {
            400 => {
                if body.contains("prompt is too long") || body.contains("Prompt is too long") {
                    let (actual, limit) = parse_prompt_too_long_counts(body);
                    ApiErrorType::PromptTooLong {
                        actual_tokens: actual,
                        limit_tokens: limit,
                        message: body.to_string(),
                    }
                } else if body.contains("credit balance") {
                    ApiErrorType::CreditBalanceTooLow {
                        message: body.to_string(),
                    }
                } else {
                    ApiErrorType::InvalidRequest {
                        message: body.to_string(),
                    }
                }
            }
            401 => {
                if body.contains("revoked") {
                    ApiErrorType::TokenRevoked {
                        message: body.to_string(),
                    }
                } else {
                    ApiErrorType::Authentication {
                        message: body.to_string(),
                    }
                }
            }
            403 => {
                if body.contains("disabled organization") {
                    ApiErrorType::OrganizationDisabled {
                        message: body.to_string(),
                    }
                } else {
                    ApiErrorType::Permission {
                        message: body.to_string(),
                    }
                }
            }
            404 => ApiErrorType::NotFound {
                message: body.to_string(),
            },
            429 => ApiErrorType::RateLimit {
                retry_after_ms: None,
                message: body.to_string(),
            },
            500 => ApiErrorType::InternalServerError {
                message: body.to_string(),
            },
            503 => ApiErrorType::ServiceUnavailable {
                message: body.to_string(),
            },
            529 => ApiErrorType::Overloaded {
                message: body.to_string(),
            },
            _ => ApiErrorType::Unknown {
                status: Some(status),
                message: body.to_string(),
            },
        }
    }
}

impl fmt::Display for ApiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.error_type)
    }
}

impl std::error::Error for ApiError {}

/// Parse "prompt is too long: X tokens > Y maximum" from error body.
fn parse_prompt_too_long_counts(body: &str) -> (Option<u64>, Option<u64>) {
    // Look for pattern: "prompt is too long" followed by numbers
    let lower = body.to_lowercase();
    if let Some(pos) = lower.find("prompt is too long") {
        let rest = &body[pos..];
        // Try to extract two numbers
        let numbers: Vec<u64> = rest
            .chars()
            .filter(|c| c.is_ascii_digit() || *c == ' ')
            .collect::<String>()
            .split_whitespace()
            .filter_map(|s| s.parse().ok())
            .collect();
        if numbers.len() >= 2 {
            return (Some(numbers[0]), Some(numbers[1]));
        }
    }
    (None, None)
}

/// Check if an error message indicates a 529 overload.
pub fn is_529_error(error: &QueryError) -> bool {
    matches!(
        error,
        QueryError::Api(ApiError {
            error_type: ApiErrorType::Overloaded { .. },
            ..
        })
    )
}

/// Check if an error is a connection-level error.
pub fn is_connection_error(error: &QueryError) -> bool {
    matches!(error, QueryError::Connection { .. })
}

/// Check if an error is a timeout.
pub fn is_timeout_error(error: &QueryError) -> bool {
    matches!(error, QueryError::Timeout)
}
