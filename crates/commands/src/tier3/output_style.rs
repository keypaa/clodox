use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct OutputStyleCommand;

impl OutputStyleCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for OutputStyleCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for OutputStyleCommand {
    fn name(&self) -> &str {
        "output"
    }

    fn description(&self) -> &str {
        "Set output style (default or minimal)"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        match args.trim() {
            "" => CommandResult::text("Current output style: default\nUsage: /output <default|minimal>"),
            "default" | "minimal" => CommandResult::text(format!("Output style set to {}", args.trim())),
            _ => CommandResult::error("Usage: /output <default|minimal>"),
        }
    }
}
