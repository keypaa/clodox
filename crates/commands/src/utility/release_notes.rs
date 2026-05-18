use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ReleaseNotesCommand;

impl ReleaseNotesCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReleaseNotesCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ReleaseNotesCommand {
    fn name(&self) -> &str {
        "release_notes"
    }

    fn description(&self) -> &str {
        "Show release notes"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /release_notes command not yet implemented")
    }
}
