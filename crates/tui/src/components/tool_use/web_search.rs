/// WebSearch tool display details.
///
/// userFacingName: "WebSearch"
/// Details: search query
/// Status: "Searching…"

/// Get the user-facing name for the WebSearch tool.
pub fn user_facing_name() -> &'static str {
    "WebSearch"
}

/// Get the display details for the WebSearch tool.
/// Shows the search query, truncated to 100 chars.
pub fn display_details(query: &str) -> String {
    if query.len() > 100 {
        format!("{}…", &query[..99])
    } else {
        query.to_string()
    }
}

/// Get the status text for the WebSearch tool.
pub fn status_text(is_running: bool, waiting_for_permission: bool) -> Option<&'static str> {
    if waiting_for_permission {
        Some("Waiting for permission…")
    } else if is_running {
        Some("Searching…")
    } else {
        None
    }
}

/// Get the activity description for the spinner.
pub fn activity_description(query: &str) -> String {
    let truncated = if query.len() > 40 {
        format!("{}…", &query[..39])
    } else {
        query.to_string()
    };
    format!("Searching {}", truncated)
}
