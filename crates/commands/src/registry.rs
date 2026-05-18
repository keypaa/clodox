use std::collections::HashMap;
use std::sync::Arc;

use crate::traits::{AuthType, Command};

/// Command registry — stores, looks up, and filters commands.
pub struct CommandRegistry {
    commands: HashMap<String, Arc<dyn Command>>,
    aliases: HashMap<String, String>,
    user_auth: Option<AuthType>,
}

impl CommandRegistry {
    pub fn new() -> Self {
        Self {
            commands: HashMap::new(),
            aliases: HashMap::new(),
            user_auth: None,
        }
    }

    pub fn with_auth(mut self, auth: AuthType) -> Self {
        self.user_auth = Some(auth);
        self
    }

    /// Register a command.
    pub fn register(&mut self, command: Arc<dyn Command>) {
        let name = command.name().to_string();
        for alias in command.aliases() {
            self.aliases.insert(alias.to_string(), name.clone());
        }
        self.commands.insert(name, command);
    }

    /// Get a command by name or alias.
    pub fn get(&self, name: &str) -> Option<&Arc<dyn Command>> {
        // Try direct lookup first
        if let Some(cmd) = self.commands.get(name) {
            if cmd.is_enabled() && self.is_available(cmd.as_ref()) {
                return Some(cmd);
            }
        }

        // Try alias lookup
        if let Some(real_name) = self.aliases.get(name) {
            if let Some(cmd) = self.commands.get(real_name) {
                if cmd.is_enabled() && self.is_available(cmd.as_ref()) {
                    return Some(cmd);
                }
            }
        }

        None
    }

    /// Get all enabled, available commands.
    pub fn get_commands(&self) -> Vec<&Arc<dyn Command>> {
        self.commands
            .values()
            .filter(|cmd| cmd.is_enabled() && !cmd.is_hidden() && self.is_available(cmd.as_ref()))
            .collect()
    }

    /// Get all commands including hidden ones.
    pub fn get_all_commands(&self) -> Vec<&Arc<dyn Command>> {
        self.commands
            .values()
            .filter(|cmd| cmd.is_enabled() && self.is_available(cmd.as_ref()))
            .collect()
    }

    /// Get model-visible commands (not disabled for model invocation).
    pub fn get_skill_tool_commands(&self) -> Vec<&Arc<dyn Command>> {
        self.commands
            .values()
            .filter(|cmd| {
                cmd.is_enabled()
                    && !cmd.is_hidden()
                    && self.is_available(cmd.as_ref())
                    && cmd.command_type() == crate::traits::CommandType::Prompt
            })
            .collect()
    }

    /// Fuzzy match command names for autocomplete.
    pub fn fuzzy_match(&self, query: &str, limit: usize) -> Vec<&Arc<dyn Command>> {
        if query.is_empty() {
            return self.get_commands().into_iter().take(limit).collect();
        }

        let query_lower = query.to_lowercase();
        let mut scored: Vec<(f64, &Arc<dyn Command>)> = self
            .get_commands()
            .into_iter()
            .filter_map(|cmd| {
                let name = cmd.name().to_lowercase();
                let name_score = strsim::jaro_winkler(&query_lower, &name);
                let alias_score = cmd.aliases()
                    .iter()
                    .map(|a| strsim::jaro_winkler(&query_lower, &a.to_lowercase()))
                    .fold(0.0, f64::max);
                let best_score = name_score.max(alias_score);
                if best_score > 0.6 {
                    Some((best_score, cmd))
                } else {
                    None
                }
            })
            .collect();

        scored.sort_by(|a, b| b.0.total_cmp(&a.0));
        scored.into_iter().take(limit).map(|(_, cmd)| cmd).collect()
    }

    /// Check if a command is available for the current user auth.
    fn is_available(&self, cmd: &dyn Command) -> bool {
        crate::traits::meets_availability(cmd, self.user_auth.as_ref())
    }

    /// Get command count.
    pub fn len(&self) -> usize {
        self.commands.len()
    }

    /// Check if registry is empty.
    pub fn is_empty(&self) -> bool {
        self.commands.is_empty()
    }
}

impl Default for CommandRegistry {
    fn default() -> Self {
        Self::new()
    }
}
