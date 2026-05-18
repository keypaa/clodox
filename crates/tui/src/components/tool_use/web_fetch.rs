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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_facing_name() {
        assert_eq!(user_facing_name(), "WebFetch");
    }

    #[test]
    fn test_display_details_short_url() {
        let url = "https://example.com";
        assert_eq!(display_details(url), url.to_string());
    }

    #[test]
    fn test_display_details_long_url() {
        let url = format!("https://example.com/{}", "a".repeat(200));
        let details = display_details(&url);
        assert_eq!(details.chars().count(), 120);
        assert!(details.ends_with("…"));
    }

    #[test]
    fn test_status_text_running() {
        assert_eq!(status_text(true, false), Some("Fetching…"));
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
    fn test_activity_description_short_url() {
        let url = "https://example.com";
        assert_eq!(activity_description(url), "Fetching https://example.com");
    }

    #[test]
    fn test_activity_description_long_url() {
        let url = format!("https://example.com/{}", "a".repeat(100));
        let desc = activity_description(&url);
        assert!(desc.starts_with("Fetching https://example.com/"));
        assert!(desc.contains("…"));
    }
}

