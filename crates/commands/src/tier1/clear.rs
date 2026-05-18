use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ClearCommand;

impl ClearCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ClearCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ClearCommand {
    fn name(&self) -> &str {
        "clear"
    }

    fn description(&self) -> &str {
        "Clear the conversation history"
    }

    fn aliases(&self) -> &[&str] {
        &["cls"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let mut state = ctx.state.write().expect("state lock poisoned");
        let message_count = state.messages.len();
        state.messages.clear();
        state.token_counts = cc_core::state::TokenCounts::default();
        drop(state);

        CommandResult::text(format!("Conversation cleared. {} messages removed.", message_count))
    }
}
