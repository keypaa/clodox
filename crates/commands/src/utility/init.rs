use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct InitCommand;

impl InitCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for InitCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for InitCommand {
    fn name(&self) -> &str {
        "init"
    }

    fn description(&self) -> &str {
        "Initialize project"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /init command not yet implemented")
    }
}
