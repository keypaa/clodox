use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct RateLimitOptionsCommand;

impl RateLimitOptionsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RateLimitOptionsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for RateLimitOptionsCommand {
    fn name(&self) -> &str {
        "rate_limit_options"
    }

    fn description(&self) -> &str {
        "Show rate limit configuration"
    }

    fn aliases(&self) -> &[&str] {
        &["rate"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let output = r#"Rate limit configuration:

  Requests per minute:    50
  Tokens per minute:      100,000
  Max concurrent:         5
  Retry on 429:           Yes (exponential backoff)
  Max retries:            3

(Rate limit configuration not yet adjustable)"#;
        CommandResult::text(output.to_string())
    }
}
