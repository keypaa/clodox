use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct RenameCommand;

impl RenameCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RenameCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for RenameCommand {
    fn name(&self) -> &str {
        "rename"
    }

    fn description(&self) -> &str {
        "Rename the current session"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        if args.trim().is_empty() {
            let state = ctx.state.read().expect("state lock poisoned");
            let current = state.session_id.as_deref().unwrap_or("unnamed");
            CommandResult::text(format!("Current session: {}\nUsage: /rename <new-name>", current))
        } else {
            CommandResult::text(format!("Session renamed to: {}\n(Session rename requires backend support)", args.trim()))
        }
    }
}
