use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ShareCommand;

impl ShareCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ShareCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ShareCommand {
    fn name(&self) -> &str {
        "share"
    }

    fn description(&self) -> &str {
        "Share a conversation"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /share command not yet implemented")
    }
}
