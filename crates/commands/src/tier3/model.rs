use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

const KNOWN_MODELS: &[(&str, &str)] = &[
    ("claude-sonnet-4-20250514", "Claude Sonnet 4 (default)"),
    ("claude-opus-4-20250514", "Claude Opus 4"),
    ("claude-sonnet-4-5-20250929", "Claude Sonnet 4.5"),
    ("claude-3-5-sonnet-20241022", "Claude 3.5 Sonnet"),
    ("claude-3-5-haiku-20241022", "Claude 3.5 Haiku"),
];

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
        "Change the model"
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
                let provider = &state.main_loop_model.provider;

                let mut output = String::from("Current model:\n\n");
                output.push_str(&format!("  {} ({})\n", current, provider));

                if let Some(session_model) = &state.main_loop_model_for_session {
                    output.push_str(&format!("  Session override: {} ({})\n", session_model.name, session_model.provider));
                }

                output.push_str("\nAvailable models:\n\n");
                for (id, desc) in KNOWN_MODELS {
                    let marker = if *id == current { " ← current" } else { "" };
                    output.push_str(&format!("  {} — {}{}\n", id, desc, marker));
                }

                output.push_str("\nUsage: /model <model-id>\n");
                CommandResult::text(output)
            }
            model => {
                let known = KNOWN_MODELS.iter().find(|(id, _)| *id == model);
                let description = known.map(|(_, d)| *d).unwrap_or("custom model");

                let mut state = ctx.state.write().expect("state lock poisoned");
                let prev = state.main_loop_model.name.clone();
                state.main_loop_model.name = model.to_string();
                state.main_loop_model.provider = "anthropic".to_string();
                drop(state);

                CommandResult::text(format!("Model changed: {} → {}\n  ({})", prev, model, description))
            }
        }
    }
}
