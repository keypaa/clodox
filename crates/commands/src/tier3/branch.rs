use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct BranchCommand;

impl BranchCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BranchCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for BranchCommand {
    fn name(&self) -> &str {
        "branch"
    }

    fn description(&self) -> &str {
        "Create a branch"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /branch command not yet implemented")
    }
}
