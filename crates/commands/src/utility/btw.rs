use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct BtwCommand;

impl BtwCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BtwCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for BtwCommand {
    fn name(&self) -> &str {
        "btw"
    }

    fn description(&self) -> &str {
        "Toggle 'by the way' suggestions"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /btw command not yet implemented")
    }
}
