use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct TagCommand;

impl TagCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TagCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for TagCommand {
    fn name(&self) -> &str {
        "tag"
    }

    fn description(&self) -> &str {
        "Tag a conversation"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /tag command not yet implemented")
    }
}
