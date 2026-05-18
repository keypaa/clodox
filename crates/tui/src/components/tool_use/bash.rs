/// Bash tool display details.
///
/// userFacingName: "Bash" or "SandboxedBash"
/// Details: command (truncated 2 lines, 160 chars)
/// Status: "Running…" / "Waiting for permission…"

/// Get the user-facing name for the Bash tool.
pub fn user_facing_name(is_sandboxed: bool) -> &'static str {
    if is_sandboxed {
        "SandboxedBash"
    } else {
        "Bash"
    }
}

/// Get the display details for the Bash tool.
/// Truncates command to 2 lines, 160 chars.
pub fn display_details(command: &str) -> String {
    // Truncate to 160 chars
    let truncated = if command.len() > 160 {
        format!("{}…", &command[..159])
    } else {
        command.to_string()
    };

    // Truncate to 2 lines
    let lines: Vec<&str> = truncated.lines().collect();
    if lines.len() > 2 {
        format!("{}…", lines[0..2].join("\n"))
    } else {
        truncated
    }
}

/// Get the status text for the Bash tool.
pub fn status_text(is_running: bool, waiting_for_permission: bool) -> Option<&'static str> {
    if waiting_for_permission {
        Some("Waiting for permission…")
    } else if is_running {
        Some("Running…")
    } else {
        None
    }
}

/// Get the activity description for the spinner.
pub fn activity_description(command: &str, description: Option<&str>) -> String {
    if let Some(desc) = description {
        format!("Running {}", desc)
    } else {
        // Truncate command for spinner
        let truncated = if command.len() > 30 {
            format!("{}…", &command[..29])
        } else {
            command.to_string()
        };
        format!("Running {}", truncated)
    }
}
