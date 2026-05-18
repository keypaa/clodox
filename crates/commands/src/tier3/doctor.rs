use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct DoctorCommand;

impl DoctorCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for DoctorCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for DoctorCommand {
    fn name(&self) -> &str {
        "doctor"
    }

    fn description(&self) -> &str {
        "Run diagnostic checks"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /doctor command not yet implemented")
    }
}
