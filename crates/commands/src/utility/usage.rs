use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct UsageCommand;

impl UsageCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UsageCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for UsageCommand {
    fn name(&self) -> &str {
        "usage"
    }

    fn description(&self) -> &str {
        "Show usage statistics"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /usage command not yet implemented")
    }
}
