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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_facing_name() {
        assert_eq!(user_facing_name(), "WebSearch");
    }

    #[test]
    fn test_display_details_short_query() {
        assert_eq!(display_details("Rust"), "Rust");
    }

    #[test]
    fn test_display_details_long_query() {
        let query = "a".repeat(200);
        let details = display_details(&query);
        assert_eq!(details.chars().count(), 100);
        assert!(details.ends_with("…"));
    }

    #[test]
    fn test_status_text_running() {
        assert_eq!(status_text(true, false), Some("Searching…"));
    }

    #[test]
    fn test_status_text_waiting() {
        assert_eq!(status_text(false, true), Some("Waiting for permission…"));
    }

    #[test]
    fn test_status_text_waiting_takes_priority() {
        assert_eq!(status_text(true, true), Some("Waiting for permission…"));
    }

    #[test]
    fn test_status_text_idle() {
        assert_eq!(status_text(false, false), None);
    }

    #[test]
    fn test_activity_description_short_query() {
        assert_eq!(activity_description("Rust"), "Searching Rust");
    }

    #[test]
    fn test_activity_description_long_query() {
        let query = format!("Rust {}", "a".repeat(100));
        let desc = activity_description(&query);
        assert!(desc.starts_with("Searching Rust "));
        assert!(desc.contains("…"));
    }
}

