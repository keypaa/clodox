use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct TerminalSetupCommand;

impl TerminalSetupCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TerminalSetupCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for TerminalSetupCommand {
    fn name(&self) -> &str {
        "terminal_setup"
    }

    fn description(&self) -> &str {
        "Show terminal configuration"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        let shell = std::env::var("SHELL").unwrap_or_else(|_| "unknown".to_string());
        let term = std::env::var("TERM").unwrap_or_else(|_| "unknown".to_string());
        let cols = std::env::var("COLUMNS").unwrap_or_else(|_| "80".to_string());
        let rows = std::env::var("LINES").unwrap_or_else(|_| "24".to_string());

        let output = format!(
            r#"Terminal configuration:

  Shell:     {}
  TERM:      {}
  Size:      {}x{}
  Truecolor: Yes
  Unicode:   Yes
  Mouse:     Enabled

(Terminal setup is automatic — no configuration needed)"#,
            shell, term, cols, rows
        );
        CommandResult::text(output)
    }
}
