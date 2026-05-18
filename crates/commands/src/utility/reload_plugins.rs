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
        "Reload all plugins"
    }

    fn aliases(&self) -> &[&str] {
        &["reload"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let count = state.plugins.enabled.len() + state.plugins.disabled.len();
        drop(state);

        CommandResult::text(format!("Reloading {} plugins...\n(Plugin reload not yet implemented)", count))
    }
}
