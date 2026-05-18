use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ThinkbackCommand;

impl ThinkbackCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ThinkbackCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ThinkbackCommand {
    fn name(&self) -> &str {
        "thinkback"
    }

    fn description(&self) -> &str {
        "Review past decisions"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /thinkback command not yet implemented")
    }
}
