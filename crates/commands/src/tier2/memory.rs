use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct MemoryCommand;

impl MemoryCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for MemoryCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for MemoryCommand {
    fn name(&self) -> &str {
        "memory"
    }

    fn description(&self) -> &str {
        "Manage CLAUDE.md memory files"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let cwd = match std::env::current_dir() {
            Ok(p) => p,
            Err(e) => return CommandResult::error(format!("Cannot get current directory: {}", e)),
        };

        match args.trim() {
            "" | "show" => {
                let mut output = String::from("Memory files (CLAUDE.md):\n\n");
                let mut found = false;

                let paths = [
                    cwd.join("CLAUDE.md"),
                    cwd.join(".claude").join("CLAUDE.md"),
                    cwd.join("CLAUDE.local.md"),
                ];

                for p in &paths {
                    if p.exists() {
                        found = true;
                        let content = std::fs::read_to_string(p).unwrap_or_else(|_| "<cannot read>".to_string());
                        let preview: String = content.lines().take(10).collect::<Vec<_>>().join("\n");
                        output.push_str(&format!("  {} ({} bytes)\n", p.display(), content.len()));
                        output.push_str(&format!("  ---\n{}\n  ---\n\n", preview));
                    }
                }

                if !found {
                    output.push_str("  No CLAUDE.md files found.\n");
                    output.push_str("\nCreate one with: /memory create <content>\n");
                }

                CommandResult::text(output)
            }
            "create" => {
                let path = cwd.join("CLAUDE.md");
                if path.exists() {
                    CommandResult::error("CLAUDE.md already exists. Use /memory edit instead.")
                } else {
                    CommandResult::text(format!("Usage: /memory create <content>\nWould create: {}", path.display()))
                }
            }
            "edit" => {
                let path = cwd.join("CLAUDE.md");
                if !path.exists() {
                    CommandResult::error("CLAUDE.md does not exist. Use /memory create instead.")
                } else {
                    CommandResult::text(format!("Usage: /memory edit <new content>\nWould edit: {}", path.display()))
                }
            }
            "clear" => {
                let path = cwd.join("CLAUDE.md");
                if path.exists() {
                    CommandResult::text(format!("Would clear: {}", path.display()))
                } else {
                    CommandResult::text("No CLAUDE.md to clear.")
                }
            }
            _ => CommandResult::error("Usage: /memory <show|create|edit|clear>"),
        }
    }
}
