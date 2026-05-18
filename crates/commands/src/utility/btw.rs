use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct BtwCommand;

impl BtwCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BtwCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for BtwCommand {
    fn name(&self) -> &str {
        "btw"
    }

    fn description(&self) -> &str {
        "By the way — share a side note"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        if args.trim().is_empty() {
            CommandResult::text("Usage: /btw <your side note>")
        } else {
            CommandResult::text(format!("Noted: {}\n\n(This is a side note and won't affect the main conversation)", args.trim()))
        }
    }
}
