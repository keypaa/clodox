use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ColorCommand;

impl ColorCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ColorCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ColorCommand {
    fn name(&self) -> &str {
        "color"
    }

    fn description(&self) -> &str {
        "Toggle colored output"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("Color output is always enabled in TUI mode. Use terminal settings to disable.")
    }
}
