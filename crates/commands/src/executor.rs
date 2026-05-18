use crate::registry::CommandRegistry;
use crate::traits::{CommandContext, CommandResult};

/// Parse a raw input string to extract slash command.
/// Returns (command_name, args) if it's a slash command, None otherwise.
pub fn parse_slash_command(input: &str) -> Option<(String, String)> {
    let trimmed = input.trim();
    if !trimmed.starts_with('/') {
        return None;
    }

    // Remove leading '/'
    let rest = &trimmed[1..];

    // Split into command and args
    if let Some(space_idx) = rest.find(' ') {
        let cmd = rest[..space_idx].to_string();
        let args = rest[space_idx + 1..].to_string();
        Some((cmd, args))
    } else {
        Some((rest.to_string(), String::new()))
    }
}

/// Execute a slash command through the registry.
pub async fn execute_command(
    registry: &CommandRegistry,
    ctx: &CommandContext,
    input: &str,
) -> CommandResult {
    let Some((name, args)) = parse_slash_command(input) else {
        return CommandResult::Skip;
    };

    let Some(cmd) = registry.get(&name) else {
        return CommandResult::text(format!("Unknown command: /{name}\nType /help to see available commands."));
    };

    cmd.execute(&args, ctx).await
}

/// Check if input is a slash command (without executing).
pub fn is_slash_command(input: &str) -> bool {
    parse_slash_command(input).is_some()
}

/// Get command suggestions for autocomplete.
pub fn get_suggestions(
    registry: &CommandRegistry,
    partial: &str,
    limit: usize,
) -> Vec<String> {
    let query = partial.strip_prefix('/').unwrap_or(partial);
    registry
        .fuzzy_match(query, limit)
        .iter()
        .map(|cmd| format!("/{}", cmd.name()))
        .collect()
}
