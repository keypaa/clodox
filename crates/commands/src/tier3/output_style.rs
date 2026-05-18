use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct OutputStyleCommand;

impl OutputStyleCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OutputStyleCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for OutputStyleCommand {
    fn name(&self) -> &str {
        "output_style"
    }

    fn description(&self) -> &str {
        "Change output style"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /output_style command not yet implemented")
    }
}
