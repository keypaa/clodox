use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct TasksCommand;

impl TasksCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for TasksCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for TasksCommand {
    fn name(&self) -> &str {
        "tasks"
    }

    fn description(&self) -> &str {
        "View and manage background tasks"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, _ctx: &CommandContext) -> CommandResult {
        CommandResult::text("TODO: /tasks command not yet implemented")
    }
}
