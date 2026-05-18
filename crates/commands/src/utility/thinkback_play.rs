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
        "Replay past conversation"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let count = state.messages.len();
        CommandResult::text(format!("Replaying {} messages...\n(Thinkback play not yet implemented)", count))
    }
}
