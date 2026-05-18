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
        "Configure privacy settings"
    }

    fn aliases(&self) -> &[&str] {
        &["privacy"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let output = r#"Privacy settings:

  Data collection:    Minimal (only session data)
  Telemetry:          Disabled
  Crash reporting:    Disabled
  API logging:        Local only

To change settings, edit your configuration file.
(Privacy settings configuration not yet implemented)"#;
        CommandResult::text(output.to_string())
    }
}
