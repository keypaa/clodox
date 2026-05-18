use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct VimCommand;

impl VimCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VimCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for VimCommand {
    fn name(&self) -> &str {
        "vim"
    }

    fn description(&self) -> &str {
        "Toggle vim mode"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /vim command not yet implemented")
    }
}
