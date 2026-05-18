use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct SandboxToggleCommand;

impl SandboxToggleCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SandboxToggleCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for SandboxToggleCommand {
    fn name(&self) -> &str {
        "sandbox"
    }

    fn description(&self) -> &str {
        "Toggle sandbox mode"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let perms = &state.worker_sandbox_permissions;

        match args.trim() {
            "" => {
                let mut output = String::from("Sandbox status:\n\n");
                if perms.is_empty() {
                    output.push_str("  Sandbox: disabled\n");
                } else {
                    output.push_str("  Sandbox: enabled\n\n  Sandboxed tools:\n");
                    for (tool, rules) in perms {
                        output.push_str(&format!("    {} — {} rules\n", tool, rules.len()));
                    }
                }
                output.push_str("\nUsage: /sandbox <on|off|status>");
                CommandResult::text(output)
            }
            "on" | "enable" => CommandResult::text("Sandbox enabled\n(Sandbox enforcement not yet implemented)"),
            "off" | "disable" => CommandResult::text("Sandbox disabled"),
            "status" => {
                CommandResult::text(format!("Sandbox: {} tools configured", perms.len()))
            }
            _ => CommandResult::error("Usage: /sandbox <on|off|status>"),
        }
    }
}
