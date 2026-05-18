use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct LogoutCommand;

impl LogoutCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LogoutCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for LogoutCommand {
    fn name(&self) -> &str {
        "logout"
    }

    fn description(&self) -> &str {
        "Clear authentication state and API key"
    }

    fn aliases(&self) -> &[&str] {
        &["signout", "sign-out"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        if let Ok(mut guard) = std::env::var("ANTHROPIC_API_KEY") {
            guard.clear();
        }

        CommandResult::text("Logged out. Authentication state cleared.")
    }
}
