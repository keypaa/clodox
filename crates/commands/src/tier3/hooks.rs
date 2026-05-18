use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct HooksCommand;

impl HooksCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for HooksCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for HooksCommand {
    fn name(&self) -> &str {
        "hooks"
    }

    fn description(&self) -> &str {
        "Show hooks status"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let hooks = &state.session_hooks_state;
        let mut output = String::from("Hooks status:\n\n");
        output.push_str(&format!("  Registered hooks: {}\n", hooks.registered_hooks.len()));
        for hook in &hooks.registered_hooks {
            output.push_str(&format!("    - {}\n", hook));
        }
        if hooks.registered_hooks.is_empty() {
            output.push_str("  No hooks registered.\n");
        }
        CommandResult::text(output)
    }
}
