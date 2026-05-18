use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::env;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoginAction {
    Submit(String),
    Cancel,
}

#[derive(Debug, Clone)]
pub struct LoginScreen {
    pub api_key_input: String,
    pub cursor_pos: usize,
    pub error_message: Option<String>,
    pub is_validating: bool,
    pub selected_action: Option<LoginAction>,
}

impl LoginScreen {
    pub fn new() -> Self {
        let existing_key = env::var("ANTHROPIC_API_KEY").ok();
        let cursor_pos = existing_key.as_ref().map_or(0, |k| k.len());
        Self {
            api_key_input: existing_key.unwrap_or_default(),
            cursor_pos,
            error_message: None,
            is_validating: false,
            selected_action: None,
        }
    }

    pub fn handle_key(&mut self, key: &str) -> Option<LoginAction> {
        match key {
            "enter" => {
                if self.api_key_input.trim().is_empty() {
                    self.error_message = Some("API key cannot be empty".to_string());
                    return None;
                }

                if !self.api_key_input.starts_with("sk-ant-")
                    && !self.api_key_input.starts_with("ant-")
                {
                    self.error_message =
                        Some("Invalid API key format (should start with sk-ant-)".to_string());
                    return None;
                }

                self.is_validating = true;
                let key = self.api_key_input.clone();
                self.selected_action = Some(LoginAction::Submit(key.clone()));
                Some(LoginAction::Submit(key))
            }
            "esc" => {
                self.selected_action = Some(LoginAction::Cancel);
                Some(LoginAction::Cancel)
            }
            "backspace" => {
                if self.cursor_pos > 0 {
                    self.api_key_input.remove(self.cursor_pos - 1);
                    self.cursor_pos -= 1;
                    self.error_message = None;
                }
                None
            }
            "delete" => {
                if self.cursor_pos < self.api_key_input.len() {
                    self.api_key_input.remove(self.cursor_pos);
                    self.error_message = None;
                }
                None
            }
            "left" => {
                if self.cursor_pos > 0 {
                    self.cursor_pos -= 1;
                }
                None
            }
            "right" => {
                if self.cursor_pos < self.api_key_input.len() {
                    self.cursor_pos += 1;
                }
                None
            }
            "home" => {
                self.cursor_pos = 0;
                None
            }
            "end" => {
                self.cursor_pos = self.api_key_input.len();
                None
            }
            _ if key.len() == 1 => {
                let c = key.chars().next().unwrap();
                if c.is_ascii_graphic() || c == ' ' {
                    self.api_key_input.insert(self.cursor_pos, c);
                    self.cursor_pos += 1;
                    self.error_message = None;
                }
                None
            }
            _ => None,
        }
    }

    fn masked_key(&self) -> String {
        if self.api_key_input.is_empty() {
            return String::new();
        }

        let len = self.api_key_input.len();
        if len <= 12 {
            "•".repeat(len)
        } else {
            let prefix = &self.api_key_input[..7];
            let suffix = &self.api_key_input[len.saturating_sub(4)..];
            format!("{}{}{}", prefix, "•".repeat(len - 11), suffix)
        }
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![Span::styled("Authenticate", title_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        let msg_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled("Enter your Anthropic API key:", msg_style),
        ]));
        lines.push(Line::from(vec![Span::raw("")]));

        let input_style = Style::default().fg(ratatui::style::Color::Green);
        let masked = self.masked_key();

        let mut input_spans = vec![Span::raw("> ")];

        if self.api_key_input.is_empty() {
            let placeholder_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);
            input_spans.push(Span::styled("sk-ant-...", placeholder_style));
            input_spans.push(Span::styled(
                " ",
                Style::default()
                    .bg(ratatui::style::Color::White)
                    .fg(ratatui::style::Color::Black),
            ));
        } else {
            let before = &masked[..self.cursor_pos.min(masked.len())];
            let after = &masked[self.cursor_pos.min(masked.len())..];

            input_spans.push(Span::styled(before.to_string(), input_style));

            if self.cursor_pos < masked.len() {
                let cursor_char: String = after.chars().take(1).collect();
                input_spans.push(Span::styled(
                    cursor_char,
                    Style::default()
                        .bg(ratatui::style::Color::White)
                        .fg(ratatui::style::Color::Green),
                ));
                let rest: String = after.chars().skip(1).collect();
                if !rest.is_empty() {
                    input_spans.push(Span::styled(rest, input_style));
                }
            } else {
                input_spans.push(Span::styled(
                    " ",
                    Style::default()
                        .bg(ratatui::style::Color::White)
                        .fg(ratatui::style::Color::Green),
                ));
            }
        }

        lines.push(Line::from(input_spans));
        lines.push(Line::from(vec![Span::raw("")]));

        if let Some(ref error) = self.error_message {
            let error_style = Style::default()
                .fg(ratatui::style::Color::Red)
                .add_modifier(Modifier::BOLD);
            lines.push(Line::from(vec![
                Span::styled(format!("⚠ {}", error), error_style),
            ]));
            lines.push(Line::from(vec![Span::raw("")]));
        }

        if self.is_validating {
            let validating_style = Style::default()
                .fg(ratatui::style::Color::Yellow)
                .add_modifier(Modifier::DIM);
            lines.push(Line::from(vec![
                Span::styled("Validating API key...", validating_style),
            ]));
            lines.push(Line::from(vec![Span::raw("")]));
        }

        let help_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![
            Span::styled(
                "Your key is stored locally and never sent to third parties.",
                help_style,
            ),
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
            Span::styled("Submit", label_style),
            Span::raw("    "),
            Span::styled("[esc] ", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", label_style),
        ]));

        lines
    }
}

impl Default for LoginScreen {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LoginScreenWidget<'a> {
    screen: &'a LoginScreen,
    theme: &'a Theme,
}

impl<'a> LoginScreenWidget<'a> {
    pub fn new(screen: &'a LoginScreen, theme: &'a Theme) -> Self {
        Self { screen, theme }
    }
}

impl Themeable for LoginScreenWidget<'_> {
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
        let dialog_height = lines.len() as u16 + 4;
        let dialog_width = 60.min(area.width);

        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x,
            y,
            width: dialog_width,
            height: dialog_height,
        };

        let border_style = Style::default().fg(ratatui::style::Color::Cyan);

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
                    let content_idx = dy.saturating_sub(2);
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

impl Widget for LoginScreenWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
