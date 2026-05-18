use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct PlanCommand;

impl PlanCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PlanCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PlanCommand {
    fn name(&self) -> &str {
        "plan"
    }

    fn description(&self) -> &str {
        "Show or manage the current plan"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        if let Some(plan) = &state.ultra_plan_state {
            let mut output = String::from("Current plan:\n\n");
            for step in &plan.steps {
                let status = match step.status {
                    cc_core::state::UltraPlanStepStatus::Pending => "○",
                    cc_core::state::UltraPlanStepStatus::InProgress => "◐",
                    cc_core::state::UltraPlanStepStatus::Completed => "●",
                    cc_core::state::UltraPlanStepStatus::Failed => "✗",
                };
                output.push_str(&format!("  {} {}\n", status, step.description));
            }
            CommandResult::text(output)
        } else {
            CommandResult::text("No active plan.\n\nUse /plan to create one. (Plan creation not yet implemented)")
        }
    }
}
