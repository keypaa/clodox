use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct IdeCommand;

impl IdeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for IdeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for IdeCommand {
    fn name(&self) -> &str {
        "ide"
    }

    fn description(&self) -> &str {
        "IDE integration"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /ide command not yet implemented")
    }
}
