use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ModelCommand;

impl ModelCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ModelCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ModelCommand {
    fn name(&self) -> &str {
        "model"
    }

    fn description(&self) -> &str {
        "Change the current model"
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
                let current = &state.main_loop_model.name;
                CommandResult::text(format!("Current model: {}\nUsage: /model <model-name>", current))
            }
            model => {
                let mut state = ctx.state.write().expect("state lock poisoned");
                let prev = state.main_loop_model.name.clone();
                state.main_loop_model.name = model.to_string();
                drop(state);
                CommandResult::text(format!("Model changed from {} to {}", prev, model))
            }
        }
    }
}
