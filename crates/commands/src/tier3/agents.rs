use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct AgentsCommand;

impl AgentsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for AgentsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for AgentsCommand {
    fn name(&self) -> &str {
        "agents"
    }

    fn description(&self) -> &str {
        "Manage agents"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        match args.trim() {
            "" | "list" => {
                let mut output = String::from("Registered agents:\n\n");

                if state.agent_name_registry.is_empty() {
                    output.push_str("  No agents registered.\n");
                } else {
                    for (name, id) in &state.agent_name_registry {
                        output.push_str(&format!("  {} — {}\n", name, id.0));
                    }
                }

                if let Some(viewing) = &state.viewing_agent_task_id {
                    output.push_str(&format!("\nCurrently viewing: {}\n", viewing));
                }
                if let Some(fg) = &state.foregrounded_task_id {
                    output.push_str(&format!("Foreground task: {}\n", fg));
                }

                output.push_str("\nUsage: /agents <list|create|kill>\n");
                CommandResult::text(output)
            }
            "create" => {
                CommandResult::text("Usage: /agents create <name> — (agent creation requires service layer)")
            }
            "kill" => {
                CommandResult::text("Usage: /agents kill <id> — (agent killing requires service layer)")
            }
            _ => CommandResult::error("Usage: /agents <list|create|kill>"),
        }
    }
}
