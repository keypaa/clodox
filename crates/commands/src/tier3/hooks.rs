use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct HooksCommand;

impl HooksCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HooksCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for HooksCommand {
    fn name(&self) -> &str {
        "hooks"
    }

    fn description(&self) -> &str {
        "Manage hooks"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /hooks command not yet implemented")
    }
}
