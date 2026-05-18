use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct PassesCommand;

impl PassesCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PassesCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PassesCommand {
    fn name(&self) -> &str {
        "passes"
    }

    fn description(&self) -> &str {
        "Show compilation passes"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("Compilation passes:\n\n  No passes configured.\n\n(Passes are used for multi-step code transformations)\n(Passes not yet implemented)")
    }
}
