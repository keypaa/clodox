use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct VoiceCommand;

impl VoiceCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VoiceCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for VoiceCommand {
    fn name(&self) -> &str {
        "voice"
    }

    fn description(&self) -> &str {
        "Toggle voice mode"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("Voice mode requires:\n  - Audio capture device\n  - Speech-to-text engine\n  - Text-to-speech engine\n\n(Voice mode not yet implemented)")
    }
}
