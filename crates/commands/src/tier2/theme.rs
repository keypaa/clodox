use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ThemeCommand;

impl ThemeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ThemeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ThemeCommand {
    fn name(&self) -> &str {
        "theme"
    }

    fn description(&self) -> &str {
        "Change the UI theme"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /theme command not yet implemented")
    }
}
