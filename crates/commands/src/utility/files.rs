use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct FilesCommand;

impl FilesCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FilesCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for FilesCommand {
    fn name(&self) -> &str {
        "files"
    }

    fn description(&self) -> &str {
        "List files in context"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /files command not yet implemented")
    }
}
