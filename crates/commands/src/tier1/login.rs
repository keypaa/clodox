use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct LoginCommand;

impl LoginCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for LoginCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for LoginCommand {
    fn name(&self) -> &str {
        "login"
    }

    fn description(&self) -> &str {
        "Authenticate with your Anthropic API key"
    }

    fn aliases(&self) -> &[&str] {
        &["auth"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Tui
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::Navigate {
            screen: "login".to_string(),
        }
    }
}
