use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct AgentsCommand;

impl AgentsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for AgentsCommand {
    fn name(&self) -> &str {
        "agents"
    }

    fn description(&self) -> &str {
        "Manage agents"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /agents command not yet implemented")
    }
}
