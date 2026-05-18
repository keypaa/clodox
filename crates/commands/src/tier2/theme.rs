use async_trait::async_trait;

use crate::traits::{Command, CommandContext, CommandResult, CommandType};
use cc_core::types::ThemeName;

pub struct ThemeCommand;

impl ThemeCommand {
    pub fn new() -> Self {
        Self
    }
}

impl Default for ThemeCommand {
    fn default() -> Self {
        Self::new()
    }
}

#[async_trait]
impl Command for ThemeCommand {
    fn name(&self) -> &str {
        "theme"
    }

    fn description(&self) -> &str {
        "Change the UI theme"
    }

    fn aliases(&self) -> &[&str] {
        &[]
    }

    fn command_type(&self) -> CommandType {
        CommandType::Local
    }

    async fn execute(&self, args: &str, ctx: &CommandContext) -> CommandResult {
        let theme = match args.trim() {
            "dark" => ThemeName::Dark,
            "light" => ThemeName::Light,
            "auto" => ThemeName::System,
            "" => {
                let state = ctx.state.read().expect("state lock poisoned");
                let current = state.settings.theme.unwrap_or_default();
                let name = match current {
                    ThemeName::Light => "light",
                    ThemeName::Dark => "dark",
                    ThemeName::System => "auto",
                };
                return CommandResult::text(format!("Current theme: {}\nUsage: /theme <dark|light|auto>", name));
            }
            _ => return CommandResult::error("Usage: /theme <dark|light|auto>"),
        };

        let mut state = ctx.state.write().expect("state lock poisoned");
        state.settings.theme = Some(theme);
        let name = match theme {
            ThemeName::Light => "light",
            ThemeName::Dark => "dark",
            ThemeName::System => "auto",
        };
        CommandResult::text(format!("Theme set to {}", name))
    }
}
