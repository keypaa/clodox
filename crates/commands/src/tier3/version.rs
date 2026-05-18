use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct VersionCommand;

impl VersionCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for VersionCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for VersionCommand {
    fn name(&self) -> &str {
        "version"
    }

    fn description(&self) -> &str {
        "Show version information"
    }

    fn aliases(&self) -> &[&str] {
        &["v"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("claude-code-rs v0.1.0\nRust port of Claude Code")
    }
}
