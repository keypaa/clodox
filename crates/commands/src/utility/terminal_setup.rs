use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct TerminalSetupCommand;

impl TerminalSetupCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalSetupCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for TerminalSetupCommand {
    fn name(&self) -> &str {
        "terminal_setup"
    }

    fn description(&self) -> &str {
        "Configure terminal setup"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /terminal_setup command not yet implemented")
    }
}
