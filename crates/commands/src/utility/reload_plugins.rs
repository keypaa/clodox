use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ReloadPluginsCommand;

impl ReloadPluginsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReloadPluginsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ReloadPluginsCommand {
    fn name(&self) -> &str {
        "reload_plugins"
    }

    fn description(&self) -> &str {
        "Reload plugins"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /reload_plugins command not yet implemented")
    }
}
