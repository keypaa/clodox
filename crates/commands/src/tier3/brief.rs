use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct BriefCommand;

impl BriefCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BriefCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for BriefCommand {
    fn name(&self) -> &str {
        "brief"
    }

    fn description(&self) -> &str {
        "Toggle brief output"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /brief command not yet implemented")
    }
}
