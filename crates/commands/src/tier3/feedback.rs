use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct FeedbackCommand;

impl FeedbackCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for FeedbackCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for FeedbackCommand {
    fn name(&self) -> &str {
        "feedback"
    }

    fn description(&self) -> &str {
        "Send feedback"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /feedback command not yet implemented")
    }
}
