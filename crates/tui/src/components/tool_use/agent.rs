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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_user_facing_name() {
        assert_eq!(user_facing_name(), "Agent");
    }

    #[test]
    fn test_display_details_with_type() {
        assert_eq!(display_details("Review code", Some("code-reviewer")), "Review code (code-reviewer)");
    }

    #[test]
    fn test_display_details_without_type() {
        assert_eq!(display_details("Do some work", None), "Do some work");
    }

    #[test]
    fn test_status_text_running() {
        assert_eq!(status_text(true, false, false, false), Some("Running…".to_string()));
    }

    #[test]
    fn test_status_text_async() {
        assert_eq!(status_text(false, true, false, false), Some("Running in background…".to_string()));
    }

    #[test]
    fn test_status_text_completed() {
        assert_eq!(status_text(false, false, false, true), Some("Completed".to_string()));
    }

    #[test]
    fn test_status_text_waiting() {
        assert_eq!(status_text(false, false, true, false), Some("Waiting for permission…".to_string()));
    }

    #[test]
    fn test_status_text_waiting_takes_priority() {
        assert_eq!(status_text(true, true, true, false), Some("Waiting for permission…".to_string()));
    }

    #[test]
    fn test_status_text_completed_takes_priority_over_running() {
        assert_eq!(status_text(true, false, false, true), Some("Completed".to_string()));
    }

    #[test]
    fn test_status_text_idle() {
        assert_eq!(status_text(false, false, false, false), None);
    }

    #[test]
    fn test_activity_description_with_type() {
        assert_eq!(
            activity_description("Review the PR", Some("code-reviewer")),
            "Launching code-reviewer: Review the PR"
        );
    }

    #[test]
    fn test_activity_description_without_type() {
        assert_eq!(
            activity_description("Do some work", None),
            "Launching agent: Do some work"
        );
    }
}

