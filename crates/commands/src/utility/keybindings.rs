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
        "Show keybindings"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /keybindings command not yet implemented")
    }
}
