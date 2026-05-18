use async_trait::async_trait;
use cc_query::compaction::snip_compact;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct CompactCommand;

impl CompactCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CompactCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for CompactCommand {
    fn name(&self) -> &str {
        "compact"
    }

    fn description(&self) -> &str {
        "Compact the conversation context to save tokens"
    }

    fn aliases(&self) -> &[&str] {
        &["compress"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let max_tokens: usize = if args.is_empty() {
            50_000
        } else {
            match args.trim().parse() {
                Ok(n) => n,
                Err(_) => return CommandResult::error("Invalid token limit. Usage: /compact [max_tokens]"),
            }
        };

        let mut state = ctx.state.write().expect("state lock poisoned");
        let messages = state.messages.clone();
        let (compacted, compaction_result) = snip_compact(&messages, max_tokens);

        if compaction_result.is_empty() {
            drop(state);
            return CommandResult::text("Conversation is already within token limits. No compaction needed.");
        }

        state.messages = compacted;
        let messages_removed = compaction_result.total_messages_removed;
        let tokens_saved = compaction_result.total_tokens_saved;
        drop(state);

        CommandResult::Compact {
            compaction_result,
            display_text: Some(format!(
                "Compacted: removed {} messages, saved ~{} tokens",
                messages_removed, tokens_saved
            )),
        }
    }
}
