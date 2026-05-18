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
    token_counts: Option<TokenDisplay>,
    cost_usd: Option<f64>,
    is_querying: bool,
}

#[derive(Debug, Clone, Default)]
pub struct TokenDisplay {
    pub input: u64,
    pub output: u64,
    pub cache_read: u64,
    pub cache_creation: u64,
}

impl PromptFooter {
    pub fn new() -> Self {
        Self {
            mode: PromptMode::Default,
            is_short: false,
            is_narrow: false,
            escape_confirmation: false,
            ctrl_c_confirmation: false,
            token_counts: None,
            cost_usd: None,
            is_querying: false,
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

    pub fn with_tokens(mut self, tokens: TokenDisplay) -> Self {
        self.token_counts = Some(tokens);
        self
    }

    pub fn with_cost(mut self, cost: f64) -> Self {
        self.cost_usd = Some(cost);
        self
    }

    pub fn with_querying(mut self, is_querying: bool) -> Self {
        self.is_querying = is_querying;
        self
    }

    fn pills(&self) -> Vec<(String, String)> {
        match self.mode {
            PromptMode::Default => {
                let mut pills: Vec<(String, String)> = vec![
                    ("default".to_string(), "".to_string()),
                ];

                if self.is_querying {
                    pills.push(("ctrl+c".to_string(), "cancel".to_string()));
                } else {
                    pills.push(("ctrl+c".to_string(), "exit".to_string()));
                }

                pills.push(("ctrl+o".to_string(), "transcript".to_string()));

                if let Some(ref tokens) = self.token_counts {
                    let total = tokens.input + tokens.output + tokens.cache_read + tokens.cache_creation;
                    if total > 0 {
                        let label = format_tokens_short(total);
                        pills.push((label, "".to_string()));
                    }
                }

                if let Some(cost) = self.cost_usd {
                    if cost > 0.0 {
                        let label = format_cost(cost);
                        pills.push((label, "".to_string()));
                    }
                }

                if self.escape_confirmation {
                    pills.push(("esc".to_string(), "again to clear".to_string()));
                }

                if self.ctrl_c_confirmation && !self.is_querying {
                    pills.push(("ctrl+c".to_string(), "again to exit".to_string()));
                }

                pills
            }
            PromptMode::HistorySearch => {
                vec![
                    ("history".to_string(), "search mode".to_string()),
                    ("enter".to_string(), "select".to_string()),
                    ("esc".to_string(), "cancel".to_string()),
                ]
            }
            PromptMode::Transcript => {
                vec![
                    ("transcript".to_string(), "mode".to_string()),
                    ("ctrl+e".to_string(), "show all".to_string()),
                    ("ctrl+o".to_string(), "exit".to_string()),
                ]
            }
            PromptMode::PermissionDialog => {
                vec![
                    ("y".to_string(), "allow once".to_string()),
                    ("a".to_string(), "allow session".to_string()),
                    ("n".to_string(), "deny".to_string()),
                    ("esc".to_string(), "cancel".to_string()),
                ]
            }
            PromptMode::Autocomplete => {
                vec![
                    ("tab".to_string(), "accept".to_string()),
                    ("shift+tab".to_string(), "cycle".to_string()),
                    ("esc".to_string(), "dismiss".to_string()),
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

fn format_tokens_short(total: u64) -> String {
    if total >= 1_000_000 {
        format!("{:.1}M tok", total as f64 / 1_000_000.0)
    } else if total >= 1_000 {
        format!("{:.1}K tok", total as f64 / 1_000.0)
    } else {
        format!("{} tok", total)
    }
}

fn format_cost(cost: f64) -> String {
    if cost >= 1.0 {
        format!("${:.2}", cost)
    } else if cost >= 0.01 {
        format!("${:.3}", cost)
    } else {
        format!("${:.4}", cost)
    }
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
