use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct EffortCommand;

impl EffortCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EffortCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for EffortCommand {
    fn name(&self) -> &str {
        "effort"
    }

    fn description(&self) -> &str {
        "Set effort level"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /effort command not yet implemented")
    }
}
