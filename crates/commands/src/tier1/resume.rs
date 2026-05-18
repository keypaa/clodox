use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ResumeCommand;

impl ResumeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ResumeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ResumeCommand {
    fn name(&self) -> &str {
        "resume"
    }

    fn description(&self) -> &str {
        "Resume a previous conversation session"
    }

    fn aliases(&self) -> &[&str] {
        &["continue"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Tui
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::Navigate {
            screen: "resume".to_string(),
        }
    }
}
