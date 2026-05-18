use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct SkillsCommand;

impl SkillsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for SkillsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for SkillsCommand {
    fn name(&self) -> &str {
        "skills"
    }

    fn description(&self) -> &str {
        "List and manage skills"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let state = ctx.state.read().expect("state lock poisoned");
        let plugins = &state.plugins;

        match args.trim() {
            "" | "list" => {
                let mut output = String::from("Installed skills:\n\n");

                if !plugins.enabled.is_empty() {
                    output.push_str("  Enabled:\n");
                    for plugin in &plugins.enabled {
                        let desc = plugin.manifest.description.as_deref().unwrap_or("No description");
                        output.push_str(&format!("    {} ({}) — {}\n", plugin.name, plugin.version, desc));
                    }
                }

                if !plugins.disabled.is_empty() {
                    output.push_str("\n  Disabled:\n");
                    for plugin in &plugins.disabled {
                        output.push_str(&format!("    {} ({})\n", plugin.name, plugin.version));
                    }
                }

                if plugins.enabled.is_empty() && plugins.disabled.is_empty() {
                    output.push_str("  No skills installed.\n");
                }

                if !plugins.errors.is_empty() {
                    output.push_str("\n  Errors:\n");
                    for err in &plugins.errors {
                        output.push_str(&format!("    {}: {}\n", err.plugin_name, err.error));
                    }
                }

                CommandResult::text(output)
            }
            "install" => {
                CommandResult::text("Usage: /skills install <skill-name>\n(Skill installation not yet implemented)")
            }
            "remove" => {
                CommandResult::text("Usage: /skills remove <skill-name>\n(Skill removal not yet implemented)")
            }
            _ => CommandResult::error("Usage: /skills <list|install|remove>"),
        }
    }
}
