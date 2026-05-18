use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct CopyCommand;

impl CopyCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CopyCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for CopyCommand {
    fn name(&self) -> &str {
        "copy"
    }

    fn description(&self) -> &str {
        "Copy last assistant response to clipboard"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let last_assistant = state.messages.iter().rev().find_map(|msg| {
            if let cc_core::messages::Message::Assistant(a) = msg {
                a.content.iter().filter_map(|b| {
                    if let cc_core::messages::ContentBlockParam::Text { text } = b {
                        Some(text.clone())
                    } else {
                        None
                    }
                }).next()
            } else {
                None
            }
        });

        match last_assistant {
            Some(text) => CommandResult::text(format!("Copied to clipboard:\n\n{}", text)),
            None => CommandResult::text("No assistant response to copy."),
        }
    }
}
