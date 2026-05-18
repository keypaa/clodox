use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct UpgradeCommand;

impl UpgradeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UpgradeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for UpgradeCommand {
    fn name(&self) -> &str {
        "upgrade"
    }

    fn description(&self) -> &str {
        "Check for updates"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("Current version: v0.1.0\n\nTo upgrade:\n  cargo install claude-code-rs\n\n(Automatic upgrade not yet implemented)")
    }
}
