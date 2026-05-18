use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct McpCommand;

impl McpCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for McpCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for McpCommand {
    fn name(&self) -> &str {
        "mcp"
    }

    fn description(&self) -> &str {
        "Manage MCP servers"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let mcp = &state.mcp;

        match args.trim() {
            "" | "status" => {
                let mut output = String::from("MCP servers:\n\n");
                if mcp.clients.is_empty() {
                    output.push_str("  No MCP servers configured.\n");
                } else {
                    for client in &mcp.clients {
                        let status = if client.connected { "connected" } else { "disconnected" };
                        output.push_str(&format!("  {} — {}\n", client.name, status));
                    }
                }
                output.push_str(&format!("\nTotal tools available: {}\n", mcp.tools.len()));
                if !mcp.commands.is_empty() {
                    output.push_str(&format!("Total commands: {}\n", mcp.commands.len()));
                }
                CommandResult::text(output)
            }
            "list" => {
                let mut output = String::from("MCP tools:\n\n");
                for tool in mcp.tools.iter() {
                    output.push_str(&format!("  {}\n", tool.name()));
                }
                if mcp.tools.is_empty() {
                    output.push_str("  No tools available.\n");
                }
                CommandResult::text(output)
            }
            "resources" => {
                let mut output = String::from("MCP resources:\n\n");
                for (server, resources) in &mcp.resources {
                    output.push_str(&format!("  {}:\n", server));
                    for r in resources {
                        output.push_str(&format!("    {} ({})\n", r.name, r.uri));
                    }
                }
                if mcp.resources.is_empty() {
                    output.push_str("  No resources available.\n");
                }
                CommandResult::text(output)
            }
            _ => CommandResult::error("Usage: /mcp <status|list|resources>"),
        }
    }
}
