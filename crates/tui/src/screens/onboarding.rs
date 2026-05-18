use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OnboardingStep {
    Welcome,
    ApiKey,
    PermissionMode,
    Theme,
    Complete,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum OnboardingAction {
    Next,
    Back,
    SelectApiKey,
    SelectPermissionMode(String),
    SelectTheme(String),
    Complete,
    Cancel,
}

#[derive(Debug, Clone)]
pub struct OnboardingScreen {
    pub current_step: OnboardingStep,
    pub username: Option<String>,
    pub api_key_selected: bool,
    pub selected_permission_mode: usize,
    pub selected_theme: usize,
    pub selected_action: Option<OnboardingAction>,
}

const PERMISSION_MODES: &[(&str, &str)] = &[
    ("Default", "Ask for permission before running commands"),
    ("Accept Edits", "Auto-approve file edits, ask for commands"),
    ("Bypass", "No permission prompts (use with caution)"),
];

const THEMES: &[(&str, &str)] = &[
    ("Dark", "Dark background, light text"),
    ("Light", "Light background, dark text"),
    ("Auto", "Detect from terminal colors"),
];

impl OnboardingScreen {
    pub fn new() -> Self {
        Self {
            current_step: OnboardingStep::Welcome,
            username: None,
            api_key_selected: false,
            selected_permission_mode: 0,
            selected_theme: 0,
            selected_action: None,
        }
    }

    pub fn handle_key(&mut self, key: &str) -> Option<OnboardingAction> {
        match self.current_step {
            OnboardingStep::Welcome => {
                match key {
                    "enter" | " " => {
                        self.current_step = OnboardingStep::ApiKey;
                        self.selected_action = Some(OnboardingAction::Next);
                        Some(OnboardingAction::Next)
                    }
                    "esc" => {
                        self.selected_action = Some(OnboardingAction::Cancel);
                        Some(OnboardingAction::Cancel)
                    }
                    _ => None,
                }
            }
            OnboardingStep::ApiKey => {
                match key {
                    "enter" | " " => {
                        self.api_key_selected = true;
                        self.current_step = OnboardingStep::PermissionMode;
                        self.selected_action = Some(OnboardingAction::SelectApiKey);
                        Some(OnboardingAction::SelectApiKey)
                    }
                    "esc" => {
                        self.selected_action = Some(OnboardingAction::Cancel);
                        Some(OnboardingAction::Cancel)
                    }
                    _ => None,
                }
            }
            OnboardingStep::PermissionMode => {
                match key {
                    "up" | "k" => {
                        if self.selected_permission_mode > 0 {
                            self.selected_permission_mode -= 1;
                        }
                        None
                    }
                    "down" | "j" => {
                        if self.selected_permission_mode + 1 < PERMISSION_MODES.len() {
                            self.selected_permission_mode += 1;
                        }
                        None
                    }
                    "enter" | " " => {
                        let mode = PERMISSION_MODES[self.selected_permission_mode].0.to_string();
                        self.current_step = OnboardingStep::Theme;
                        self.selected_action = Some(OnboardingAction::SelectPermissionMode(mode));
                        self.selected_action.clone()
                    }
                    "esc" => {
                        self.selected_action = Some(OnboardingAction::Cancel);
                        Some(OnboardingAction::Cancel)
                    }
                    _ => None,
                }
            }
            OnboardingStep::Theme => {
                match key {
                    "up" | "k" => {
                        if self.selected_theme > 0 {
                            self.selected_theme -= 1;
                        }
                        None
                    }
                    "down" | "j" => {
                        if self.selected_theme + 1 < THEMES.len() {
                            self.selected_theme += 1;
                        }
                        None
                    }
                    "enter" | " " => {
                        let theme = THEMES[self.selected_theme].0.to_string();
                        self.current_step = OnboardingStep::Complete;
                        self.selected_action = Some(OnboardingAction::SelectTheme(theme));
                        self.selected_action.clone()
                    }
                    "esc" => {
                        self.selected_action = Some(OnboardingAction::Cancel);
                        Some(OnboardingAction::Cancel)
                    }
                    _ => None,
                }
            }
            OnboardingStep::Complete => {
                match key {
                    "enter" | " " => {
                        self.selected_action = Some(OnboardingAction::Complete);
                        Some(OnboardingAction::Complete)
                    }
                    _ => None,
                }
            }
        }
    }

    fn render_clawd(&self) -> Vec<Line<'static>> {
        let body_color = ratatui::style::Color::Cyan;
        let bg_color = ratatui::style::Color::Rgb(30, 30, 30);

        let body_style = Style::default().fg(body_color);
        let body_bg_style = Style::default().fg(body_color).bg(bg_color);

        vec![
            Line::from(vec![
                Span::styled(" ▐", body_style),
                Span::styled("▛███▜", body_bg_style),
                Span::styled("▌", body_style),
            ]),
            Line::from(vec![
                Span::styled("▝▜", body_style),
                Span::styled("█████", body_bg_style),
                Span::styled("▛▘", body_style),
            ]),
            Line::from(vec![
                Span::styled("  ▘▘ ▝▝  ", body_style),
            ]),
        ]
    }

    fn render_welcome(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        lines.extend(self.render_clawd());
        lines.push(Line::from(vec![Span::raw("")]));

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![Span::styled("Welcome to Claude Code", title_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        let desc_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("An agentic coding agent that lives in your terminal.", desc_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));
        lines.push(Line::from(vec![
            Span::styled("It understands your codebase, writes and edits files,", desc_style),
        ]));
        lines.push(Line::from(vec![
            Span::styled("runs commands, and helps you ship faster.", desc_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let hint_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("Press Enter to get started", hint_style),
        ]));

        lines
    }

    fn render_api_key(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![Span::styled("Step 1: API Key", title_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        let desc_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("Claude Code requires an Anthropic API key.", desc_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let env_style = Style::default()
            .fg(ratatui::style::Color::Yellow)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![
            Span::styled("Option 1: Set ANTHROPIC_API_KEY environment variable", env_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let or_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![Span::styled("OR", or_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        let input_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![
            Span::styled("Option 2: Run /login to enter your key", input_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let hint_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("Get your key at console.anthropic.com", hint_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let key_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);

        lines.push(Line::from(vec![
            Span::styled("[enter] ", key_style),
            Span::styled("Continue", label_style),
            Span::raw("    "),
            Span::styled("[esc] ", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", label_style),
        ]));

        lines
    }

    fn render_permission_mode(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![Span::styled("Step 2: Permission Mode", title_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        let desc_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("How should Claude handle tool execution?", desc_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        for (i, (name, desc)) in PERMISSION_MODES.iter().enumerate() {
            let is_selected = i == self.selected_permission_mode;

            let prefix = if is_selected { "▸ " } else { "  " };
            let num_style = if is_selected {
                Style::default()
                    .fg(ratatui::style::Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            };
            let name_style = if is_selected {
                Style::default()
                    .fg(ratatui::style::Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(ratatui::style::Color::White)
                    .add_modifier(Modifier::DIM)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{}[{}] ", prefix, i + 1), num_style),
                Span::styled(name.to_string(), name_style),
            ]));

            let detail_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);
            lines.push(Line::from(vec![
                Span::styled(format!("    {}", desc), detail_style),
            ]));
        }

        lines.push(Line::from(vec![Span::raw("")]));

        let key_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);

        lines.push(Line::from(vec![
            Span::styled("[enter] ", key_style),
            Span::styled("Select", label_style),
            Span::raw("    "),
            Span::styled("[↑↓] ", key_style),
            Span::styled("Navigate", label_style),
            Span::raw("    "),
            Span::styled("[esc] ", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", label_style),
        ]));

        lines
    }

    fn render_theme_selection(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![Span::styled("Step 3: Theme", title_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        let desc_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("Choose your preferred color scheme:", desc_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        for (i, (name, desc)) in THEMES.iter().enumerate() {
            let is_selected = i == self.selected_theme;

            let prefix = if is_selected { "▸ " } else { "  " };
            let num_style = if is_selected {
                Style::default()
                    .fg(ratatui::style::Color::Green)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM)
            };
            let name_style = if is_selected {
                Style::default()
                    .fg(ratatui::style::Color::White)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default()
                    .fg(ratatui::style::Color::White)
                    .add_modifier(Modifier::DIM)
            };

            lines.push(Line::from(vec![
                Span::styled(format!("{}[{}] ", prefix, i + 1), num_style),
                Span::styled(name.to_string(), name_style),
            ]));

            let detail_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);
            lines.push(Line::from(vec![
                Span::styled(format!("    {}", desc), detail_style),
            ]));
        }

        lines.push(Line::from(vec![Span::raw("")]));

        let key_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);

        lines.push(Line::from(vec![
            Span::styled("[enter] ", key_style),
            Span::styled("Select", label_style),
            Span::raw("    "),
            Span::styled("[↑↓] ", key_style),
            Span::styled("Navigate", label_style),
            Span::raw("    "),
            Span::styled("[esc] ", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", label_style),
        ]));

        lines
    }

    fn render_complete(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![Span::styled("✓ Setup Complete!", title_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        let desc_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("You're all set! Start coding with Claude.", desc_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let tip_style = Style::default()
            .fg(ratatui::style::Color::Yellow)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("Tip: Use /help to see all available commands.", tip_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let key_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);

        lines.push(Line::from(vec![
            Span::styled("[enter] ", key_style),
            Span::styled("Start coding!", label_style),
        ]));

        lines
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        match self.current_step {
            OnboardingStep::Welcome => self.render_welcome(),
            OnboardingStep::ApiKey => self.render_api_key(),
            OnboardingStep::PermissionMode => self.render_permission_mode(),
            OnboardingStep::Theme => self.render_theme_selection(),
            OnboardingStep::Complete => self.render_complete(),
        }
    }
}

impl Default for OnboardingScreen {
    fn default() -> Self {
        Self::new()
    }
}

pub struct OnboardingWidget<'a> {
    screen: &'a OnboardingScreen,
    theme: &'a Theme,
}

impl<'a> OnboardingWidget<'a> {
    pub fn new(screen: &'a OnboardingScreen, theme: &'a Theme) -> Self {
        Self { screen, theme }
    }
}

impl Themeable for OnboardingWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        // Clear the entire screen first to remove any leftover content from previous modes
        let bg_style = Style::default().bg(ratatui::style::Color::Black);
        for y in 0..area.height {
            for x in 0..area.width {
                let bx = area.x + x;
                let by = area.y + y;
                if bx >= buf.area.width || by >= buf.area.height {
                    continue;
                }
                if let Some(cell) = buf.cell_mut((bx, by)) {
                    cell.set_symbol(" ");
                    cell.set_style(bg_style);
                }
            }
        }

        let lines = self.screen.render_lines();
        let dialog_height = lines.len() as u16 + 2;
        let dialog_width = 60.min(area.width);

        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x,
            y,
            width: dialog_width,
            height: dialog_height,
        };

        let border_color = match self.screen.current_step {
            OnboardingStep::Complete => ratatui::style::Color::Green,
            _ => ratatui::style::Color::Cyan,
        };
        let border_style = Style::default().fg(border_color);

        for dx in 0..dialog_width {
            for dy in 0..dialog_height {
                let bx = dialog_area.x + dx;
                let by = dialog_area.y + dy;
                if bx >= buf.area.width || by >= buf.area.height {
                    continue;
                }

                let cell = buf.cell_mut((bx, by)).unwrap();

                if dx == 0 && dy == 0 {
                    cell.set_symbol("┌");
                    cell.set_style(border_style);
                } else if dx == dialog_width - 1 && dy == 0 {
                    cell.set_symbol("┐");
                    cell.set_style(border_style);
                } else if dx == 0 && dy == dialog_height - 1 {
                    cell.set_symbol("└");
                    cell.set_style(border_style);
                } else if dx == dialog_width - 1 && dy == dialog_height - 1 {
                    cell.set_symbol("┘");
                    cell.set_style(border_style);
                } else if dx == 0 || dx == dialog_width - 1 {
                    cell.set_symbol("│");
                    cell.set_style(border_style);
                } else if dy == 0 || dy == dialog_height - 1 {
                    cell.set_symbol("─");
                    cell.set_style(border_style);
                } else {
                    let content_idx = dy - 1;
                    if content_idx < lines.len() as u16 {
                        let line = &lines[content_idx as usize];
                        let mut char_idx = 0u16;
                        for span in &line.spans {
                            for ch in span.content.chars() {
                                if char_idx + 3 < dialog_width {
                                    let cell_x = dialog_area.x + char_idx + 2;
                                    if cell_x < buf.area.width {
                                        if let Some(c) = buf.cell_mut((cell_x, by)) {
                                            c.set_symbol(&ch.to_string());
                                            c.set_style(span.style);
                                        }
                                    }
                                }
                                char_idx += 1;
                            }
                        }
                    } else {
                        cell.set_symbol(" ");
                    }
                }
            }
        }
    }
}

impl Widget for OnboardingWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
