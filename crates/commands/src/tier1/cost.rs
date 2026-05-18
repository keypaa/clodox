use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct CostCommand;

impl CostCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for CostCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for CostCommand {
    fn name(&self) -> &str {
        "cost"
    }

    fn description(&self) -> &str {
        "Show API cost summary for the current session"
    }

    fn aliases(&self) -> &[&str] {
        &["usage", "tokens"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        let total_cost = state.total_cost_usd;
        let input_tokens = state.token_counts.input_tokens;
        let output_tokens = state.token_counts.output_tokens;
        let cache_read = state.token_counts.cache_read_tokens;
        let cache_creation = state.token_counts.cache_creation_tokens;
        let total_tokens = input_tokens + output_tokens + cache_read + cache_creation;
        let message_count = state.messages.len();
        drop(state);

        let mut output = String::from("Session cost summary:\n\n");
        output.push_str(&format!("  Total cost:       ${:.4}\n", total_cost));
        output.push_str(&format!("  Total tokens:     {}\n", total_tokens));
        output.push_str(&format!("  Input tokens:     {}\n", input_tokens));
        output.push_str(&format!("  Output tokens:    {}\n", output_tokens));
        output.push_str(&format!("  Cache read:       {}\n", cache_read));
        output.push_str(&format!("  Cache creation:   {}\n", cache_creation));
        output.push_str(&format!("\n  Messages:         {}\n", message_count));

        CommandResult::text(output)
    }
}
