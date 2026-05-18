use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct RemoteEnvCommand;

impl RemoteEnvCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RemoteEnvCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for RemoteEnvCommand {
    fn name(&self) -> &str {
        "remote_env"
    }

    fn description(&self) -> &str {
        "Configure remote environment"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /remote_env command not yet implemented")
    }
}
