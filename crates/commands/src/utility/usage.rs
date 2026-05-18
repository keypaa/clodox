use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct UsageCommand;

impl UsageCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for UsageCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for UsageCommand {
    fn name(&self) -> &str {
        "usage"
    }

    fn description(&self) -> &str {
        "Show API usage statistics"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        let mut output = String::from("API usage (this session):\n\n");
        output.push_str(&format!("  Requests:        1+\n"));
        output.push_str(&format!("  Input tokens:    {}\n", state.token_counts.input_tokens));
        output.push_str(&format!("  Output tokens:   {}\n", state.token_counts.output_tokens));
        output.push_str(&format!("  Cache read:      {}\n", state.token_counts.cache_read_tokens));
        output.push_str(&format!("  Cache created:   {}\n", state.token_counts.cache_creation_tokens));
        output.push_str(&format!("  Total cost:      ${:.4}\n", state.total_cost_usd));
        output.push_str("\n(Usage tracking is session-only — not persisted)");
        CommandResult::text(output)
    }
}
