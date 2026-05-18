use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};
use cc_core::permissions::PermissionMode;

pub struct PermissionsCommand;

impl PermissionsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PermissionsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PermissionsCommand {
    fn name(&self) -> &str {
        "permissions"
    }

    fn description(&self) -> &str {
        "Show or change permission settings"
    }

    fn aliases(&self) -> &[&str] {
        &["perms"]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        match args.trim() {
            "" => {
                let state = ctx.state.read().expect("state lock poisoned");
                let mode = &state.tool_permission_context.mode;
                CommandResult::text(format!("Current permission mode: {:?}\nUsage: /permissions <default|accept-edits|bypass|dont-ask|plan>", mode))
            }
            "default" | "accept-edits" | "bypass" | "dont-ask" | "plan" => {
                let mode = match args.trim() {
                    "default" => PermissionMode::Default,
                    "accept-edits" => PermissionMode::AcceptEdits,
                    "bypass" => PermissionMode::BypassPermissions,
                    "dont-ask" => PermissionMode::DontAsk,
                    "plan" => PermissionMode::Plan,
                    _ => PermissionMode::default(),
                };
                let mut state = ctx.state.write().expect("state lock poisoned");
                state.tool_permission_context.mode = mode;
                drop(state);
                CommandResult::text(format!("Permission mode set to {}", args.trim()))
            }
            _ => CommandResult::error("Usage: /permissions <default|accept-edits|bypass|dont-ask|plan>"),
        }
    }
}
