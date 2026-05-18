use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct KeybindingsCommand;

impl KeybindingsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for KeybindingsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for KeybindingsCommand {
    fn name(&self) -> &str {
        "keybindings"
    }

    fn description(&self) -> &str {
        "Show keyboard shortcuts"
    }

    fn aliases(&self) -> &[&str] {
        &["keys", "shortcuts"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let output = r#"Keyboard shortcuts:

  Enter        Send message
  Ctrl+C       Cancel query / Exit (double-press)
  Ctrl+L       Clear screen
  Up/Down      Navigate history
  Tab          Autocomplete
  Ctrl+K       Clear input
  Ctrl+U       Delete to start of line
  Ctrl+W       Delete previous word
  Ctrl+R       Search history
  Esc          Close dialog / Navigate back
  ?            Show help
  /            Start slash command
"#;
        CommandResult::text(output.to_string())
    }
}
