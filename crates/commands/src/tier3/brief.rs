use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct BriefCommand;

impl BriefCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for BriefCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for BriefCommand {
    fn name(&self) -> &str {
        "brief"
    }

    fn description(&self) -> &str {
        "Toggle brief output mode"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let mut state = ctx.state.write().expect("state lock poisoned");
        state.brief_mode = !state.brief_mode;
        let mode = if state.brief_mode { "enabled" } else { "disabled" };
        CommandResult::text(format!("Brief mode {}", mode))
    }
}
