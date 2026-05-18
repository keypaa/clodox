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

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        let input_tokens = state.token_counts.input_tokens;
        let output_tokens = state.token_counts.output_tokens;
        let cache_read = state.token_counts.cache_read_tokens;
        let cache_creation = state.token_counts.cache_creation_tokens;
        let total = input_tokens + output_tokens + cache_read + cache_creation;

        let max = 200_000u64;
        let pct = (total as f64 / max as f64 * 100.0).min(100.0);

        match args.trim() {
            "" => {
                let mut output = String::from("Context usage:\n\n");
                output.push_str(&format!("  Input:        {} tokens\n", input_tokens));
                output.push_str(&format!("  Output:       {} tokens\n", output_tokens));
                output.push_str(&format!("  Cache read:   {} tokens\n", cache_read));
                output.push_str(&format!("  Cache create: {} tokens\n", cache_creation));
                output.push_str(&format!("  Total:        {} tokens ({:.1}% of {})\n", total, pct, max));
                output.push_str(&format!("  Messages:     {}\n", state.messages.len()));
                output.push_str(&format!("  Remaining:    {} tokens\n", max.saturating_sub(total)));
                CommandResult::text(output)
            }
            "compact" => {
                CommandResult::text("To compact context, use /compact")
            }
            "details" => {
                let mut output = String::from("Context details:\n\n");
                output.push_str(&format!("  Model:         {}\n", state.main_loop_model.name));
                output.push_str(&format!("  Session:       {}\n", state.session_id.as_deref().unwrap_or("none")));
                output.push_str(&format!("  Messages:      {}\n", state.messages.len()));
                output.push_str(&format!("  Tools used:    {}\n", state.pending_tool_calls.len()));
                output.push_str(&format!("  Query state:   {:?}\n", state.query_state));
                if let Some(turn) = &state.current_turn_tokens {
                    output.push_str(&format!("  Turn cost:     ${:.4}\n", turn.cost_usd));
                }
                output.push_str(&format!("  Total cost:    ${:.4}\n", state.total_cost_usd));
                CommandResult::text(output)
            }
            _ => CommandResult::error("Usage: /context <details|compact>"),
        }
    }
}
