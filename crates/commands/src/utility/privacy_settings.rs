use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct PrivacySettingsCommand;

impl PrivacySettingsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PrivacySettingsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PrivacySettingsCommand {
    fn name(&self) -> &str {
        "privacy_settings"
    }

    fn description(&self) -> &str {
        "Manage privacy settings"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /privacy_settings command not yet implemented")
    }
}
