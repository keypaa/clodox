use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct PrCommentsCommand;

impl PrCommentsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PrCommentsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PrCommentsCommand {
    fn name(&self) -> &str {
        "pr_comments"
    }

    fn description(&self) -> &str {
        "View PR comments"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /pr_comments command not yet implemented")
    }
}
