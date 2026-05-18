use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ThinkbackPlayCommand;

impl ThinkbackPlayCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ThinkbackPlayCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ThinkbackPlayCommand {
    fn name(&self) -> &str {
        "thinkback_play"
    }

    fn description(&self) -> &str {
        "Play back thinkback"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /thinkback_play command not yet implemented")
    }
}
