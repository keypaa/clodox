/// WebFetch tool display details.
///
/// userFacingName: "WebFetch"
/// Details: URL (truncated)
/// Status: "Fetching…" / "Processing…"

/// Get the user-facing name for the WebFetch tool.
pub fn user_facing_name() -> &'static str {
    "WebFetch"
}

/// Get the display details for the WebFetch tool.
/// Shows URL, truncated to 120 chars.
pub fn display_details(url: &str) -> String {
    if url.len() > 120 {
        format!("{}…", &url[..119])
    } else {
        url.to_string()
    }
}

/// Get the status text for the WebFetch tool.
pub fn status_text(is_running: bool, waiting_for_permission: bool) -> Option<&'static str> {
    if waiting_for_permission {
        Some("Waiting for permission…")
    } else if is_running {
        Some("Fetching…")
    } else {
        None
    }
}

/// Get the activity description for the spinner.
pub fn activity_description(url: &str) -> String {
    let truncated = if url.len() > 40 {
        format!("{}…", &url[..39])
    } else {
        url.to_string()
    };
    format!("Fetching {}", truncated)
}
