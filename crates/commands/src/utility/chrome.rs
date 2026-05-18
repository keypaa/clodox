use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ChromeCommand;

impl ChromeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChromeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ChromeCommand {
    fn name(&self) -> &str {
        "chrome"
    }

    fn description(&self) -> &str {
        "Chrome integration"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /chrome command not yet implemented")
    }
}
