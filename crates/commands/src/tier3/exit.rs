use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ExitCommand;

impl ExitCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ExitCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ExitCommand {
    fn name(&self) -> &str {
        "exit"
    }

    fn description(&self) -> &str {
        "Exit the application"
    }

    fn aliases(&self) -> &[&str] {
        &["quit", "q"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("Exiting...")
    }
}
