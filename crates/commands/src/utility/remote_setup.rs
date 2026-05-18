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
        "Configure remote session"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let output = r#"Remote session setup:

  To connect to a remote session:
  1. Start the bridge server on the remote machine
  2. Run: /remote_setup <bridge-url>
  3. Authenticate when prompted

  (Remote setup not yet implemented)"#;
        CommandResult::text(output.to_string())
    }
}
