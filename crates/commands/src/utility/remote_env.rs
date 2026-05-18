use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct RemoteEnvCommand;

impl RemoteEnvCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for RemoteEnvCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for RemoteEnvCommand {
    fn name(&self) -> &str {
        "remote_env"
    }

    fn description(&self) -> &str {
        "Show remote environment status"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, _args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");

        let mut output = String::from("Remote environment:\n\n");
        output.push_str(&format!("  Bridge enabled:    {}\n", state.repl_bridge_enabled));
        output.push_str(&format!("  Connected:         {}\n", state.repl_bridge_connected));
        output.push_str(&format!("  Session active:    {}\n", state.repl_bridge_session_active));
        output.push_str(&format!("  Reconnecting:      {}\n", state.repl_bridge_reconnecting));

        if let Some(url) = &state.repl_bridge_connect_url {
            output.push_str(&format!("  Connect URL:       {}\n", url));
        }
        if let Some(url) = &state.repl_bridge_session_url {
            output.push_str(&format!("  Session URL:       {}\n", url));
        }
        if let Some(id) = &state.repl_bridge_session_id {
            output.push_str(&format!("  Session ID:        {}\n", id));
        }
        if let Some(err) = &state.repl_bridge_error {
            output.push_str(&format!("  Error:             {}\n", err));
        }

        CommandResult::text(output)
    }
}
