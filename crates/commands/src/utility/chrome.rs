use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ChromeCommand;

impl ChromeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ChromeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ChromeCommand {
    fn name(&self) -> &str {
        "chrome"
    }

    fn description(&self) -> &str {
        "Open Chrome DevTools integration"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("Chrome DevTools integration requires a running Chrome instance with remote debugging enabled.\n\nUsage: chrome --remote-debugging-port=9222\n\n(Chrome integration not yet implemented)")
    }
}
