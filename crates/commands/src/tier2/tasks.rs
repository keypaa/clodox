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

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        if state.tasks.is_empty() {
            return CommandResult::text("No background tasks.");
        }

        match args.trim() {
            "" => {
                let mut output = String::from("Background tasks:\n\n");
                for (id, task) in &state.tasks {
                    output.push_str(&format!("  {} — {} ({:?})\n", id, task.name, task.status));
                }
                CommandResult::text(output)
            }
            "list" => {
                let mut output = String::from("Tasks:\n\n");
                for (id, task) in &state.tasks {
                    output.push_str(&format!("  {} | {} | {:?}\n", id, task.name, task.status));
                }
                CommandResult::text(output)
            }
            id if id.starts_with("kill ") || id.starts_with("stop ") => {
                let task_id = id.split_whitespace().skip(1).collect::<Vec<_>>().join(" ");
                if state.tasks.contains_key(&task_id) {
                    drop(state);
                    let mut state = ctx.state.write().expect("state lock poisoned");
                    if let Some(task) = state.tasks.get_mut(&task_id) {
                        use cc_core::state::TaskStatus;
                        task.status = TaskStatus::Killed;
                    }
                    drop(state);
                    CommandResult::text(format!("Task {} killed.", task_id))
                } else {
                    CommandResult::error(format!("Task not found: {}", task_id))
                }
            }
            id => {
                if let Some(task) = state.tasks.get(id) {
                    let mut output = String::from("Task details:\n\n");
                    output.push_str(&format!("  ID:     {}\n", task.id));
                    output.push_str(&format!("  Name:   {}\n", task.name));
                    output.push_str(&format!("  Status: {:?}\n", task.status));
                    output.push_str(&format!("  Created: {}\n", task.created_at));
                    if let Some(agent_id) = &task.agent_id {
                        output.push_str(&format!("  Agent:  {}\n", agent_id.0));
                    }
                    CommandResult::text(output)
                } else {
                    CommandResult::error(format!("Unknown task or command: {}", id))
                }
            }
        }
    }
}
