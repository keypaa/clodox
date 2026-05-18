use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ContextCommand;

impl ContextCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ContextCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ContextCommand {
    fn name(&self) -> &str {
        "context"
    }

    fn description(&self) -> &str {
        "Show current context usage"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let total = state.token_counts.input_tokens + state.token_counts.output_tokens;
        let max = 200_000u64;
        let pct = (total as f64 / max as f64 * 100.0).min(100.0);
        let mut output = String::from("Context usage:\n\n");
        output.push_str(&format!("  Used:     {} tokens ({:.1}%)\n", total, pct));
        output.push_str(&format!("  Maximum:  {} tokens\n", max));
        output.push_str(&format!("  Remaining: {} tokens\n", max.saturating_sub(total)));
        output.push_str(&format!("  Messages: {}\n", state.messages.len()));
        CommandResult::text(output)
    }
}
