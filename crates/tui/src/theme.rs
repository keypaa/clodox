use ratatui::style::Color;

/// Theme name for UI styling.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeName {
    #[default]
    Dark,
    Light,
}

/// Theme setting — what the user has configured.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum ThemeSetting {
    #[default]
    Dark,
    Light,
    /// Auto-detect based on terminal background.
    Auto,
}

/// Theme color palette matching TypeScript's theme system.
#[derive(Debug, Clone)]
pub struct Theme {
    pub name: ThemeName,
    pub colors: ThemeColors,
}

/// Theme color keys matching TypeScript.
#[derive(Debug, Clone)]
pub struct ThemeColors {
    /// Primary text color.
    pub text: Color,
    /// Error messages (red).
    pub error: Color,
    /// Warnings (yellow).
    pub warning: Color,
    /// Suggestions/highlights (blue).
    pub suggestion: Color,
    /// Dimmed/inactive text.
    pub inactive: Color,
    /// Inverted colors (for badges).
    pub inverse_text: Color,
    /// Message action background.
    pub message_actions_background: Color,
    /// Subtle text.
    pub subtle: Color,
    /// Success indicators (green).
    pub success: Color,
    /// User message background.
    pub user_message_background: Color,
    /// Input border color.
    pub prompt_border: Color,
    /// Brief label for "You".
    pub brief_label_you: Color,
}

impl Theme {
    /// Create the dark theme (default).
    pub fn dark() -> Self {
        Self {
            name: ThemeName::Dark,
            colors: ThemeColors {
                text: Color::White,
                error: Color::Red,
                warning: Color::Yellow,
                suggestion: Color::Blue,
                inactive: Color::Rgb(177, 173, 161),
                inverse_text: Color::White,
                message_actions_background: Color::Rgb(40, 40, 40),
                subtle: Color::Rgb(177, 173, 161),
                success: Color::Green,
                user_message_background: Color::Rgb(30, 30, 30),
                prompt_border: Color::Rgb(177, 173, 161),
                brief_label_you: Color::Green,
            },
        }
    }

    /// Create the light theme.
    pub fn light() -> Self {
        Self {
            name: ThemeName::Light,
            colors: ThemeColors {
                text: Color::Black,
                error: Color::Red,
                warning: Color::Yellow,
                suggestion: Color::Blue,
                inactive: Color::Rgb(177, 173, 161),
                inverse_text: Color::Black,
                message_actions_background: Color::Rgb(230, 230, 230),
                subtle: Color::Rgb(177, 173, 161),
                success: Color::Green,
                user_message_background: Color::Rgb(240, 240, 240),
                prompt_border: Color::Rgb(177, 173, 161),
                brief_label_you: Color::Green,
            },
        }
    }

    /// Detect theme from environment.
    /// Uses `$COLORFGBG` — if background is dark, return dark theme.
    pub fn from_env() -> ThemeSetting {
        if let Ok(colorfgbg) = std::env::var("COLORFGBG") {
            // Format: "foreground;background" or "default;foreground;background"
            let parts: Vec<&str> = colorfgbg.split(';').collect();
            if let Some(bg) = parts.last() {
                if let Ok(bg_val) = bg.parse::<i64>() {
                    // Values < 8 are typically dark colors
                    return if bg_val < 8 {
                        ThemeSetting::Dark
                    } else {
                        ThemeSetting::Light
                    };
                }
            }
        }
        ThemeSetting::Dark
    }

    /// Resolve the theme based on the setting.
    pub fn resolve(setting: ThemeSetting) -> Self {
        match setting {
            ThemeSetting::Dark => Self::dark(),
            ThemeSetting::Light => Self::light(),
            ThemeSetting::Auto => {
                let detected = Self::from_env();
                Self::resolve(detected)
            }
        }
    }
}

impl Default for Theme {
    fn default() -> Self {
        Self::dark()
    }
}

/// Get a color from the current theme.
pub fn color(theme: &Theme, key: &str) -> Color {
    match key {
        "text" => theme.colors.text,
        "error" => theme.colors.error,
        "warning" => theme.colors.warning,
        "suggestion" => theme.colors.suggestion,
        "inactive" => theme.colors.inactive,
        "inverseText" => theme.colors.inverse_text,
        "messageActionsBackground" => theme.colors.message_actions_background,
        "subtle" => theme.colors.subtle,
        "success" => theme.colors.success,
        "userMessageBackground" => theme.colors.user_message_background,
        "promptBorder" => theme.colors.prompt_border,
        "briefLabelYou" => theme.colors.brief_label_you,
        _ => theme.colors.text,
    }
}

/// Convert a theme color key to a ratatui `Style`.
pub fn style(theme: &Theme, key: &str) -> ratatui::style::Style {
    ratatui::style::Style::default().fg(color(theme, key))
}

/// Create a dim style.
pub fn dim_style(theme: &Theme) -> ratatui::style::Style {
    ratatui::style::Style::default()
        .fg(theme.colors.inactive)
        .add_modifier(ratatui::style::Modifier::DIM)
}

/// Create an error style.
pub fn error_style(theme: &Theme) -> ratatui::style::Style {
    ratatui::style::Style::default().fg(theme.colors.error)
}

/// Create a success style.
pub fn success_style(theme: &Theme) -> ratatui::style::Style {
    ratatui::style::Style::default().fg(theme.colors.success)
}

/// Create a suggestion/highlight style.
pub fn suggestion_style(theme: &Theme) -> ratatui::style::Style {
    ratatui::style::Style::default().fg(theme.colors.suggestion)
}

/// Trait for theme-aware widgets.
pub trait Themeable {
    fn render_themed(
        &self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &Theme,
    );
}

impl Themeable for ratatui::text::Line<'_> {
    fn render_themed(
        &self,
        area: ratatui::layout::Rect,
        buf: &mut ratatui::buffer::Buffer,
        _theme: &Theme,
    ) {
        ratatui::widgets::Widget::render(self.clone(), area, buf);
    }
}

impl Theme {
    /// Get a color by key name.
    pub fn color(&self, key: &str) -> Color {
        color(self, key)
    }
}
