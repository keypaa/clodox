/// Read tool display details.
///
/// userFacingName: "Read" / "Reading Plan"
/// Details: displayPath [+ "· pages N" / "· lines X-Y"]

/// Get the user-facing name for the Read tool.
pub fn user_facing_name(file_path: &str) -> &str {
    if file_path.contains("/plans/") {
        "Reading Plan"
    } else {
        "Read"
    }
}

/// Get the display details for the Read tool.
pub fn display_details(
    display_path: &str,
    pages: Option<&str>,
    offset: Option<u64>,
    limit: Option<u64>,
) -> String {
    let mut details = display_path.to_string();

    if let Some(p) = pages {
        details.push_str(&format!(" · pages {}", p));
    } else if let (Some(o), Some(l)) = (offset, limit) {
        let end = o + l - 1;
        details.push_str(&format!(" · lines {}-{}", o, end));
    } else if let Some(o) = offset {
        details.push_str(&format!(" · from line {}", o));
    }

    details
}

/// Get the activity description for the spinner.
pub fn activity_description(display_path: &str) -> String {
    format!("Reading {}", display_path)
}
