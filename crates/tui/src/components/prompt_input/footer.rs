use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone, PartialEq)]
pub enum PromptMode {
    Default,
    HistorySearch,
    Transcript,
    PermissionDialog,
    Autocomplete,
}

#[derive(Debug, Clone)]
pub struct PromptFooter {
    mode: PromptMode,
    is_short: bool,
    is_narrow: bool,
    escape_confirmation: bool,
    ctrl_c_confirmation: bool,
}

impl PromptFooter {
    pub fn new() -> Self {
        Self {
            mode: PromptMode::Default,
            is_short: false,
            is_narrow: false,
            escape_confirmation: false,
            ctrl_c_confirmation: false,
        }
    }

    pub fn with_mode(mut self, mode: PromptMode) -> Self {
        self.mode = mode;
        self
    }

    pub fn with_dimensions(mut self, is_short: bool, is_narrow: bool) -> Self {
        self.is_short = is_short;
        self.is_narrow = is_narrow;
        self
    }

    pub fn with_escape_confirmation(mut self, confirmed: bool) -> Self {
        self.escape_confirmation = confirmed;
        self
    }

    pub fn with_ctrl_c_confirmation(mut self, confirmed: bool) -> Self {
        self.ctrl_c_confirmation = confirmed;
        self
    }

    fn pills(&self) -> Vec<(&str, &str)> {
        match self.mode {
            PromptMode::Default => {
                let mut pills = vec![
                    ("default", ""),
                    ("ctrl+c", "exit"),
                    ("ctrl+o", "transcript"),
                ];

                if self.escape_confirmation {
                    pills.push(("esc", "again to clear"));
                }

                if self.ctrl_c_confirmation {
                    pills.push(("ctrl+c", "again to exit"));
                }

                pills
            }
            PromptMode::HistorySearch => {
                vec![
                    ("history", "search mode"),
                    ("enter", "select"),
                    ("esc", "cancel"),
                ]
            }
            PromptMode::Transcript => {
                vec![
                    ("transcript", "mode"),
                    ("ctrl+e", "show all"),
                    ("ctrl+o", "exit"),
                ]
            }
            PromptMode::PermissionDialog => {
                vec![
                    ("y", "allow once"),
                    ("a", "allow session"),
                    ("n", "deny"),
                    ("esc", "cancel"),
                ]
            }
            PromptMode::Autocomplete => {
                vec![
                    ("tab", "accept"),
                    ("shift+tab", "cycle"),
                    ("esc", "dismiss"),
                ]
            }
        }
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        if self.is_short {
            return vec![];
        }

        let pills = self.pills();
        let mut spans = Vec::new();

        for (i, (key, desc)) in pills.iter().enumerate() {
            if i > 0 {
                if self.is_narrow {
                    spans.push(Span::raw(" "));
                } else {
                    spans.push(Span::styled(
                        " · ",
                        Style::default()
                            .fg(theme_placeholder().colors.inactive)
                            .add_modifier(Modifier::DIM),
                    ));
                }
            }

            let key_style = Style::default()
                .fg(theme_placeholder().colors.subtle)
                .add_modifier(Modifier::DIM);

            spans.push(Span::styled(key.to_string(), key_style));

            if !desc.is_empty() {
                let desc_style = Style::default()
                    .fg(theme_placeholder().colors.inactive)
                    .add_modifier(Modifier::DIM);
                spans.push(Span::styled(format!(" {}", desc), desc_style));
            }
        }

        vec![Line::from(spans)]
    }
}

fn theme_placeholder() -> Theme {
    Theme::dark()
}

impl Themeable for PromptFooter {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        if self.is_short {
            return;
        }

        let pills = self.pills();
        let mut spans = Vec::new();

        for (i, (key, desc)) in pills.iter().enumerate() {
            if i > 0 {
                if self.is_narrow {
                    spans.push(Span::raw(" "));
                } else {
                    spans.push(Span::styled(
                        " · ",
                        Style::default()
                            .fg(theme.colors.inactive)
                            .add_modifier(Modifier::DIM),
                    ));
                }
            }

            let key_style = Style::default()
                .fg(theme.colors.subtle)
                .add_modifier(Modifier::DIM);

            spans.push(Span::styled(key.to_string(), key_style));

            if !desc.is_empty() {
                let desc_style = Style::default()
                    .fg(theme.colors.inactive)
                    .add_modifier(Modifier::DIM);
                spans.push(Span::styled(format!(" {}", desc), desc_style));
            }
        }

        let line = Line::from(spans);
        let y = area.y.min(buf.area.height.saturating_sub(1));
        let mut x = area.x;
        for span in &line.spans {
            for ch in span.content.chars() {
                if x < area.x + area.width {
                    if let Some(cell) = buf.cell_mut((x, y)) {
                        cell.set_symbol(&ch.to_string());
                        cell.set_style(span.style);
                    }
                }
                x += 1;
            }
        }
    }
}

impl Widget for PromptFooter {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
