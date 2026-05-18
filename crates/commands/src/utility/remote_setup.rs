use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct RemoteSetupCommand;

impl RemoteSetupCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RemoteSetupCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for RemoteSetupCommand {
    fn name(&self) -> &str {
        "remote_setup"
    }

    fn description(&self) -> &str {
        "Set up remote connection"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /remote_setup command not yet implemented")
    }
}
