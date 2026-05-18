use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct FastCommand;

impl FastCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FastCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for FastCommand {
    fn name(&self) -> &str {
        "fast"
    }

    fn description(&self) -> &str {
        "Toggle fast mode"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /fast command not yet implemented")
    }
}
