/// Agent tool display details.
///
/// userFacingName: "Agent"
/// Details: description + agent type
/// Status: "Launching…" / "Running…" / "Completed"

/// Get the user-facing name for the Agent tool.
pub fn user_facing_name() -> &'static str {
    "Agent"
}

/// Get the display details for the Agent tool.
/// Shows description and agent type.
pub fn display_details(description: &str, agent_type: Option<&str>) -> String {
    match agent_type {
        Some(atype) => format!("{description} ({atype})"),
        None => description.to_string(),
    }
}

/// Get the status text for the Agent tool.
pub fn status_text(
    is_running: bool,
    is_async: bool,
    waiting_for_permission: bool,
    is_completed: bool,
) -> Option<String> {
    if waiting_for_permission {
        return Some("Waiting for permission…".to_string());
    }
    if is_completed {
        return Some("Completed".to_string());
    }
    if is_async {
        return Some("Running in background…".to_string());
    }
    if is_running {
        return Some("Running…".to_string());
    }
    None
}

/// Get the activity description for the spinner.
pub fn activity_description(description: &str, agent_type: Option<&str>) -> String {
    match agent_type {
        Some(atype) => format!("Launching {atype}: {description}"),
        None => format!("Launching agent: {description}"),
    }
}
