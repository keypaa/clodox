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
        "Tag the current session"
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
            let sid = state.session_id.as_deref().unwrap_or("unnamed");
            CommandResult::text(format!("Session: {}\nUsage: /tag <tag-name>", sid))
        } else {
            CommandResult::text(format!("Tag '{}' applied to session\n(Session tagging requires backend support)", args.trim()))
        }
    }
}
