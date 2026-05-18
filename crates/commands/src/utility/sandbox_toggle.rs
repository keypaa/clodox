use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct SandboxToggleCommand;

impl SandboxToggleCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SandboxToggleCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for SandboxToggleCommand {
    fn name(&self) -> &str {
        "sandbox_toggle"
    }

    fn description(&self) -> &str {
        "Toggle sandbox mode"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /sandbox_toggle command not yet implemented")
    }
}
