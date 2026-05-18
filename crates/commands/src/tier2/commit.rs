use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct CommitCommand;

impl CommitCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CommitCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for CommitCommand {
    fn name(&self) -> &str {
        "commit"
    }

    fn description(&self) -> &str {
        "Commit changes with an AI-generated message"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /commit command not yet implemented")
    }
}
