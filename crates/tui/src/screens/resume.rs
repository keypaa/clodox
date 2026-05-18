use chrono::{DateTime, Local};
use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::fs;
use std::path::PathBuf;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone)]
pub struct SessionEntry {
    pub id: String,
    pub summary: String,
    pub last_active: DateTime<Local>,
    pub message_count: usize,
}

impl SessionEntry {
    pub fn time_ago(&self) -> String {
        let now = Local::now();
        let duration = now.signed_duration_since(self.last_active);

        if duration.num_minutes() < 1 {
            "just now".to_string()
        } else if duration.num_minutes() < 60 {
            format!("{}m ago", duration.num_minutes())
        } else if duration.num_hours() < 24 {
            format!("{}h ago", duration.num_hours())
        } else if duration.num_days() < 7 {
            format!("{}d ago", duration.num_days())
        } else {
            self.last_active.format("%b %d").to_string()
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResumePicker {
    pub sessions: Vec<SessionEntry>,
    pub selected_index: usize,
    pub selected_action: Option<ResumeAction>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResumeAction {
    Resume(usize),
    NewSession,
    Cancel,
}

impl ResumePicker {
    pub fn new(sessions: Vec<SessionEntry>) -> Self {
        Self {
            sessions,
            selected_index: 0,
            selected_action: None,
        }
    }

    pub fn load_sessions() -> Vec<SessionEntry> {
        let sessions_dir = get_sessions_dir();
        if !sessions_dir.exists() {
            return Vec::new();
        }

        let mut sessions = Vec::new();

        if let Ok(entries) = fs::read_dir(&sessions_dir) {
            for entry in entries.flatten() {
                let path = entry.path();
                if path.extension().and_then(|e| e.to_str()) == Some("json") {
                    if let Some(session) = parse_session_file(&path) {
                        sessions.push(session);
                    }
                }
            }
        }

        sessions.sort_by(|a, b| b.last_active.cmp(&a.last_active));
        sessions.truncate(10);
        sessions
    }

    pub fn handle_key(&mut self, key: &str) -> Option<ResumeAction> {
        match key {
            "up" | "k" => {
                if self.selected_index > 0 {
                    self.selected_index -= 1;
                }
                None
            }
            "down" | "j" => {
                if self.selected_index + 1 < self.sessions.len() {
                    self.selected_index += 1;
                }
                None
            }
            "enter" => {
                if self.sessions.is_empty() {
                    self.selected_action = Some(ResumeAction::NewSession);
                    Some(ResumeAction::NewSession)
                } else {
                    let idx = self.selected_index;
                    self.selected_action = Some(ResumeAction::Resume(idx));
                    Some(ResumeAction::Resume(idx))
                }
            }
            "n" => {
                self.selected_action = Some(ResumeAction::NewSession);
                Some(ResumeAction::NewSession)
            }
            "esc" => {
                self.selected_action = Some(ResumeAction::Cancel);
                Some(ResumeAction::Cancel)
            }
            _ => None,
        }
    }

    fn render_lines(&self, width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);

        lines.push(Line::from(vec![Span::styled("Resume Session", title_style)]));
        lines.push(Line::from(vec![Span::raw("")]));

        if self.sessions.is_empty() {
            let empty_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);
            lines.push(Line::from(vec![
                Span::styled("No previous sessions found.", empty_style),
            ]));
            lines.push(Line::from(vec![Span::raw("")]));
        } else {
            let max_show = 8;
            let to_show = self.sessions.iter().take(max_show);

            for (i, session) in to_show.enumerate() {
                let is_selected = i == self.selected_index;

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

                let summary_style = if is_selected {
                    Style::default()
                        .fg(ratatui::style::Color::White)
                        .add_modifier(Modifier::BOLD)
                } else {
                    Style::default()
                        .fg(ratatui::style::Color::White)
                        .add_modifier(Modifier::DIM)
                };

                let time_style = if is_selected {
                    Style::default()
                        .fg(ratatui::style::Color::Yellow)
                } else {
                    Style::default()
                        .fg(ratatui::style::Color::DarkGray)
                        .add_modifier(Modifier::DIM)
                };

                let max_summary_width = (width as usize).saturating_sub(20);
                let summary = if session.summary.len() > max_summary_width {
                    format!("{}…", &session.summary[..max_summary_width.saturating_sub(1)])
                } else {
                    session.summary.clone()
                };

                lines.push(Line::from(vec![
                    Span::styled(format!("{}[{}] ", prefix, i + 1), num_style),
                    Span::styled(summary, summary_style),
                    Span::raw("  "),
                    Span::styled(session.time_ago(), time_style),
                ]));
            }

            if self.sessions.len() > max_show {
                let more_style = Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM);
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("  ... +{} more", self.sessions.len() - max_show),
                        more_style,
                    ),
                ]));
            }

            lines.push(Line::from(vec![Span::raw("")]));
        }

        let key_style = Style::default()
            .fg(ratatui::style::Color::Green)
            .add_modifier(Modifier::BOLD);
        let label_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::DIM);

        let mut buttons = vec![
            Span::styled("[enter] ", key_style),
            Span::styled("Resume", label_style),
            Span::raw("    "),
            Span::styled("[n] ", key_style),
            Span::styled("New session", label_style),
            Span::raw("    "),
            Span::styled("[esc] ", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::BOLD)),
            Span::styled("Cancel", label_style),
        ];

        lines.push(Line::from(buttons));

        lines
    }
}

fn get_sessions_dir() -> PathBuf {
    let mut path = dirs::home_dir().unwrap_or_else(|| PathBuf::from("."));
    path.push(".claude");
    path.push("sessions");
    path
}

fn parse_session_file(path: &PathBuf) -> Option<SessionEntry> {
    let content = fs::read_to_string(path).ok()?;
    let id = path
        .file_stem()
        .and_then(|s| s.to_str())
        .unwrap_or("unknown")
        .to_string();

    let metadata = fs::metadata(path).ok()?;
    let modified = metadata.modified().ok()?;
    let last_active = DateTime::<Local>::from(modified);

    let summary = if content.len() > 200 {
        let first_line = content.lines().next().unwrap_or("");
        if first_line.len() > 100 {
            format!("{}…", &first_line[..99])
        } else {
            first_line.to_string()
        }
    } else {
        content.trim().to_string()
    };

    Some(SessionEntry {
        id,
        summary,
        last_active,
        message_count: content.lines().count(),
    })
}

pub struct ResumePickerWidget<'a> {
    picker: &'a ResumePicker,
    theme: &'a Theme,
}

impl<'a> ResumePickerWidget<'a> {
    pub fn new(picker: &'a ResumePicker, theme: &'a Theme) -> Self {
        Self { picker, theme }
    }
}

impl Themeable for ResumePickerWidget<'_> {
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

        let lines = self.picker.render_lines(area.width);
        let dialog_height = lines.len() as u16 + 2;
        let dialog_width = 70.min(area.width);

        let x = area.x + (area.width.saturating_sub(dialog_width)) / 2;
        let y = area.y + (area.height.saturating_sub(dialog_height)) / 2;

        let dialog_area = Rect {
            x,
            y,
            width: dialog_width,
            height: dialog_height,
        };

        let border_style = Style::default().fg(ratatui::style::Color::Cyan);

        // Now render the border
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

impl Widget for ResumePickerWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
