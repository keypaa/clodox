use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct RenameCommand;

impl RenameCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RenameCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for RenameCommand {
    fn name(&self) -> &str {
        "rename"
    }

    fn description(&self) -> &str {
        "Rename a session"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /rename command not yet implemented")
    }
}
