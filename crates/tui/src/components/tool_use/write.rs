/// Write tool display details.
///
/// userFacingName: "Write" / "Updated plan"
/// Details: displayPath

/// Get the user-facing name for the Write tool.
pub fn user_facing_name(file_path: &str) -> &str {
    if file_path.contains("/plans/") {
        "Updated plan"
    } else {
        "Write"
    }
}

/// Get the display details for the Write tool.
pub fn display_details(display_path: &str) -> String {
    display_path.to_string()
}

/// Get the activity description for the spinner.
pub fn activity_description(display_path: &str) -> String {
    format!("Writing {}", display_path)
}
