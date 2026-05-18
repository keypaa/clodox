use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct StatsCommand;

impl StatsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StatsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for StatsCommand {
    fn name(&self) -> &str {
        "stats"
    }

    fn description(&self) -> &str {
        "Show detailed session statistics"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let total_tokens = state.token_counts.input_tokens
            + state.token_counts.output_tokens
            + state.token_counts.cache_read_tokens
            + state.token_counts.cache_creation_tokens;
        let mut output = String::from("Session statistics:\n\n");
        output.push_str(&format!("  Input tokens:       {}\n", state.token_counts.input_tokens));
        output.push_str(&format!("  Output tokens:      {}\n", state.token_counts.output_tokens));
        output.push_str(&format!("  Cache read:         {}\n", state.token_counts.cache_read_tokens));
        output.push_str(&format!("  Cache creation:     {}\n", state.token_counts.cache_creation_tokens));
        output.push_str(&format!("  Total tokens:       {}\n", total_tokens));
        output.push_str(&format!("  Total cost:         ${:.4}\n", state.total_cost_usd));
        output.push_str(&format!("  Messages sent:      {}\n", state.messages.len()));
        CommandResult::text(output)
    }
}
