use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};
use cc_core::types::EffortValue;

pub struct EffortCommand;

impl EffortCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for EffortCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for EffortCommand {
    fn name(&self) -> &str {
        "effort"
    }

    fn description(&self) -> &str {
        "Set effort level (low, medium, high)"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        match args.trim() {
            "" => {
                let state = ctx.state.read().expect("state lock poisoned");
                CommandResult::text(format!("Current effort: {:?}\nUsage: /effort <low|medium|high>", state.effort))
            }
            "low" | "medium" | "high" => {
                let effort = match args.trim() {
                    "low" => EffortValue::Low,
                    "medium" => EffortValue::Medium,
                    "high" => EffortValue::High,
                    _ => EffortValue::default(),
                };
                let mut state = ctx.state.write().expect("state lock poisoned");
                state.effort = effort;
                drop(state);
                CommandResult::text(format!("Effort set to {}", args.trim()))
            }
            _ => CommandResult::error("Usage: /effort <low|medium|high>"),
        }
    }
}
