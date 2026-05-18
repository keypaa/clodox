use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};

pub struct PrCommentsCommand;

impl PrCommentsCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for PrCommentsCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for PrCommentsCommand {
    fn name(&self) -> &str {
        "pr_comments"
    }

    fn description(&self) -> &str {
        "View PR comments"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, _ctx: &CommandContext) -> CommandResult {
        let pr_url = args.trim();

        if pr_url.is_empty() {
            let branch = std::process::Command::new("git")
                .arg("branch")
                .arg("--show-current")
                .output();

            match branch {
                Ok(o) if o.status.success() => {
                    let current = String::from_utf8_lossy(&o.stdout).trim().to_string();
                    CommandResult::text(format!(
                        "Usage: /pr_comments <pr-url-or-number>\n\nCurrent branch: {}\nExample: /pr_comments https://github.com/org/repo/pull/123",
                        current
                    ))
                }
                _ => CommandResult::text("Usage: /pr_comments <pr-url-or-number>\nExample: /pr_comments https://github.com/org/repo/pull/123"),
            }
        } else {
            CommandResult::text(format!(
                "Fetching PR comments for: {}\n\n(PR comments require GitHub API integration — not yet implemented)",
                pr_url
            ))
        }
    }
}
