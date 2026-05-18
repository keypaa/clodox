use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct ReviewCommand;

impl ReviewCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ReviewCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ReviewCommand {
    fn name(&self) -> &str {
        "review"
    }

    fn description(&self) -> &str {
        "Review code changes"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let diff = if args.trim().is_empty() {
            std::process::Command::new("git")
                .arg("diff")
                .arg("HEAD")
                .output()
        } else {
            std::process::Command::new("git")
                .arg("diff")
                .arg(args.trim())
                .output()
        };

        match diff {
            Ok(o) if o.status.success() => {
                let output = String::from_utf8_lossy(&o.stdout);
                if output.trim().is_empty() {
                    return CommandResult::text("No changes to review.");
                }

                let lines: Vec<&str> = output.lines().collect();
                let display: Vec<&str> = lines.iter().take(100).cloned().collect();
                let mut result = String::from("Code review — changes:\n\n");
                result.push_str(&display.join("\n"));
                if lines.len() > 100 {
                    result.push_str(&format!("\n\n... ({} more lines, use /review <ref> for specific diff)", lines.len() - 100));
                }
                CommandResult::text(result)
            }
            Ok(o) => CommandResult::error(format!("Git error: {}", String::from_utf8_lossy(&o.stderr))),
            Err(e) => CommandResult::error(format!("Failed to run git: {}", e)),
        }
    }
}
