use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct StatusCommand;

impl StatusCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for StatusCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for StatusCommand {
    fn name(&self) -> &str {
        "status"
    }

    fn description(&self) -> &str {
        "Show current session status"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let mut output = String::from("Session status:\n\n");
        output.push_str(&format!("  Model:         {}\n", state.main_loop_model.name));
        output.push_str(&format!("  Messages:      {}\n", state.messages.len()));
        output.push_str(&format!("  Tokens:        {} in, {} out\n", state.token_counts.input_tokens, state.token_counts.output_tokens));
        output.push_str(&format!("  Cost:          ${:.4}\n", state.total_cost_usd));
        output.push_str(&format!("  Querying:      {}\n", state.is_querying));
        output.push_str(&format!("  Vim mode:      {}\n", state.vim_mode));
        output.push_str(&format!("  Fast mode:     {}\n", state.fast_mode));
        output.push_str(&format!("  Brief mode:    {}\n", state.brief_mode));
        if let Some(sid) = &state.session_id {
            output.push_str(&format!("  Session:       {}\n", sid));
        }
        CommandResult::text(output)
    }
}
