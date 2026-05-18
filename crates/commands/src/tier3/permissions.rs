use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct PermissionsCommand;

impl PermissionsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PermissionsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PermissionsCommand {
    fn name(&self) -> &str {
        "permissions"
    }

    fn description(&self) -> &str {
        "Manage tool permissions"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /permissions command not yet implemented")
    }
}
