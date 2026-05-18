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
        "Configure rate limits"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /rate_limit_options command not yet implemented")
    }
}
