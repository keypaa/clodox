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
        "Share the current conversation"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let msg_count = state.messages.len();
        CommandResult::text(format!("Sharing conversation with {} messages...\n(Note: sharing service not yet implemented)", msg_count))
    }
}
