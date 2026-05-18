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
        "Send feedback about the application"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        if args.trim().is_empty() {
            CommandResult::text("Usage: /feedback <your feedback message>")
        } else {
            CommandResult::text(format!("Feedback submitted:\n\n{}\n\nThank you!", args.trim()))
        }
    }
}
