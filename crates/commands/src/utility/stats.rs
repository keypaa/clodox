use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct StatsCommand;

impl StatsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StatsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for StatsCommand {
    fn name(&self) -> &str {
        "stats"
    }

    fn description(&self) -> &str {
        "Show session statistics"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /stats command not yet implemented")
    }
}
