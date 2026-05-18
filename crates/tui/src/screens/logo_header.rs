use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::path::Path;

use crate::theme::{Theme, Themeable};
use crate::state::AppState;
use crate::screens::resume::SessionEntry;

const LEFT_PANEL_MAX_WIDTH: u16 = 50;
const HORIZONTAL_MODE_THRESHOLD: u16 = 70;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LayoutMode {
    Horizontal,
    Compact,
}

pub fn get_layout_mode(columns: u16) -> LayoutMode {
    if columns >= HORIZONTAL_MODE_THRESHOLD {
        LayoutMode::Horizontal
    } else {
        LayoutMode::Compact
    }
}

#[derive(Debug, Clone)]
pub struct LogoHeader {
    pub version: String,
    pub username: Option<String>,
    pub cwd: String,
    pub model_name: String,
    pub billing_type: String,
    pub agent_name: Option<String>,
    pub recent_activity: Vec<SessionEntry>,
    pub release_notes: Vec<String>,
    pub sandbox_status: bool,
    pub debug_mode: bool,
    pub debug_log_path: Option<String>,
    pub announcement: Option<String>,
    pub organization_name: Option<String>,
}

impl LogoHeader {
    pub fn new() -> Self {
        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            username: None,
            cwd: std::env::current_dir()
                .ok()
                .map(|p| p.display().to_string())
                .unwrap_or_else(|| ".".to_string()),
            model_name: "claude-sonnet-4-20250514".to_string(),
            billing_type: "API Usage Billing".to_string(),
            agent_name: None,
            recent_activity: Vec::new(),
            release_notes: Vec::new(),
            sandbox_status: false,
            debug_mode: false,
            debug_log_path: None,
            announcement: None,
            organization_name: None,
        }
    }

    pub fn from_state(state: &AppState) -> Self {
        let cwd = std::env::current_dir()
            .ok()
            .map(|p| p.display().to_string())
            .unwrap_or_else(|| ".".to_string());

        let model_short = shorten_model(&state.main_loop_model.name);

        Self {
            version: env!("CARGO_PKG_VERSION").to_string(),
            username: None,
            cwd,
            model_name: model_short,
            billing_type: "API Usage Billing".to_string(),
            agent_name: None,
            recent_activity: Vec::new(),
            release_notes: Vec::new(),
            sandbox_status: false,
            debug_mode: false,
            debug_log_path: None,
            announcement: None,
            organization_name: None,
        }
    }

    pub fn with_username(mut self, name: &str) -> Self {
        self.username = Some(name.to_string());
        self
    }

    pub fn with_agent(mut self, name: &str) -> Self {
        self.agent_name = Some(name.to_string());
        self
    }

    pub fn with_recent_activity(mut self, entries: Vec<SessionEntry>) -> Self {
        self.recent_activity = entries;
        self
    }

    pub fn with_release_notes(mut self, notes: Vec<String>) -> Self {
        self.release_notes = notes;
        self
    }

    pub fn with_sandbox_status(mut self, enabled: bool) -> Self {
        self.sandbox_status = enabled;
        self
    }

    pub fn with_debug_mode(mut self, enabled: bool, log_path: Option<&str>) -> Self {
        self.debug_mode = enabled;
        self.debug_log_path = log_path.map(|s| s.to_string());
        self
    }

    pub fn with_announcement(mut self, text: &str, org: Option<&str>) -> Self {
        self.announcement = Some(text.to_string());
        self.organization_name = org.map(|s| s.to_string());
        self
    }

    fn welcome_message(&self) -> String {
        match &self.username {
            Some(name) if name.len() <= 20 => format!("Welcome back {}!", name),
            _ => "Welcome back!".to_string(),
        }
    }

    fn model_line(&self) -> String {
        format!("{} · {}", self.model_name, self.billing_type)
    }

    fn cwd_line(&self) -> String {
        let truncated = truncate_path(&self.cwd, 40);
        match &self.agent_name {
            Some(agent) => format!("@{} · {}", agent, truncated),
            None => truncated,
        }
    }

    fn render_lines(&self, columns: u16) -> Vec<Line<'static>> {
        let layout_mode = get_layout_mode(columns);

        match layout_mode {
            LayoutMode::Compact => self.render_compact(columns),
            LayoutMode::Horizontal => self.render_horizontal(columns),
        }
    }

    fn render_compact(&self, _columns: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let welcome = self.welcome_message();
        let welcome_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![Span::styled(welcome, welcome_style)]));

        lines.push(Line::from(vec![Span::raw("")]));

        lines.extend(self.render_clawd());

        lines.push(Line::from(vec![Span::raw("")]));

        let dim_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);
        lines.push(Line::from(vec![Span::styled(self.model_line().clone(), dim_style)]));
        lines.push(Line::from(vec![Span::styled(self.cwd_line().clone(), dim_style)]));

        if self.sandbox_status {
            lines.push(Line::from(vec![Span::raw("")]));
            let warn_style = Style::default().fg(ratatui::style::Color::Yellow);
            lines.push(Line::from(vec![
                Span::styled("Your bash commands will be sandboxed. Disable with /sandbox.", warn_style),
            ]));
        }

        lines
    }

    fn render_horizontal(&self, columns: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let left_width = LEFT_PANEL_MAX_WIDTH.min(columns / 2);
        let right_width = columns.saturating_sub(left_width + 3);

        let welcome = self.welcome_message();
        let welcome_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::BOLD);

        let clawd_lines = self.render_clawd();

        let dim_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);
        let model_line = self.model_line();
        let cwd_line = self.cwd_line();

        let activity_lines = self.render_recent_activity_feed(right_width);
        let whats_new_lines = self.render_whats_new_feed(right_width);

        let max_height = 9.max(activity_lines.len()).max(whats_new_lines.len());

        for i in 0..max_height {
            let mut left_spans = Vec::new();
            let mut right_spans = Vec::new();

            match i {
                0 => {
                    left_spans.push(Span::styled(welcome.clone(), welcome_style));
                }
                1 => {}
                2..=4 => {
                    if let Some(clawd_line) = clawd_lines.get(i - 2) {
                        for span in &clawd_line.spans {
                            left_spans.push(span.clone());
                        }
                    }
                }
                5 => {}
                6 => {
                    left_spans.push(Span::styled(model_line.clone(), dim_style));
                }
                7 => {
                    left_spans.push(Span::styled(cwd_line.clone(), dim_style));
                }
                _ => {}
            }

            if i < activity_lines.len() {
                for span in &activity_lines[i].spans {
                    right_spans.push(span.clone());
                }
            }

            let mut combined = left_spans;
            combined.push(Span::raw(" "));
            combined.push(Span::styled("│", Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::DIM)));
            combined.push(Span::raw(" "));
            combined.extend(right_spans);

            lines.push(Line::from(combined));
        }

        lines
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

    fn render_recent_activity_feed(&self, max_width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Cyan)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![Span::styled("Recent Activity", title_style)]));

        let separator = "─".repeat(max_width.min(30) as usize);
        lines.push(Line::from(vec![Span::styled(
            separator,
            Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::DIM),
        )]));

        for (i, entry) in self.recent_activity.iter().take(3).enumerate() {
            let num_style = Style::default()
                .fg(ratatui::style::Color::Green)
                .add_modifier(Modifier::BOLD);
            let summary_style = Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::DIM);
            let time_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);

            let max_summary = (max_width as usize).saturating_sub(12);
            let summary = if entry.summary.len() > max_summary {
                format!("{}…", &entry.summary[..max_summary.saturating_sub(1)])
            } else {
                entry.summary.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(format!("[{}] ", i + 1), num_style),
                Span::styled(summary, summary_style),
            ]));
            lines.push(Line::from(vec![
                Span::raw("    "),
                Span::styled(entry.time_ago(), time_style),
            ]));
        }

        if self.recent_activity.is_empty() {
            let empty_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);
            lines.push(Line::from(vec![Span::styled("No recent activity", empty_style)]));
        }

        lines
    }

    fn render_whats_new_feed(&self, max_width: u16) -> Vec<Line<'static>> {
        let mut lines = Vec::new();

        let title_style = Style::default()
            .fg(ratatui::style::Color::Yellow)
            .add_modifier(Modifier::BOLD);
        lines.push(Line::from(vec![Span::styled("What's New", title_style)]));

        let separator = "─".repeat(max_width.min(30) as usize);
        lines.push(Line::from(vec![Span::styled(
            separator,
            Style::default().fg(ratatui::style::Color::DarkGray).add_modifier(Modifier::DIM),
        )]));

        for note in self.release_notes.iter().take(3) {
            let note_style = Style::default()
                .fg(ratatui::style::Color::White)
                .add_modifier(Modifier::DIM);

            let max_note = (max_width as usize).saturating_sub(4);
            let display = if note.len() > max_note {
                format!("{}…", &note[..max_note.saturating_sub(1)])
            } else {
                note.clone()
            };

            lines.push(Line::from(vec![
                Span::styled(format!("• {}", display), note_style),
            ]));
        }

        if self.release_notes.is_empty() {
            let empty_style = Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM);
            lines.push(Line::from(vec![Span::styled("Up to date", empty_style)]));
        }

        lines
    }
}

impl Default for LogoHeader {
    fn default() -> Self {
        Self::new()
    }
}

pub struct LogoHeaderWidget<'a> {
    header: &'a LogoHeader,
    theme: &'a Theme,
    columns: u16,
}

impl<'a> LogoHeaderWidget<'a> {
    pub fn new(header: &'a LogoHeader, theme: &'a Theme, columns: u16) -> Self {
        Self { header, theme, columns }
    }
}

impl Themeable for LogoHeaderWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, _theme: &Theme) {
        let lines = self.header.render_lines(self.columns);
        let layout_mode = get_layout_mode(self.columns);

        let border_title = format!(" Claude Code v{} ", self.header.version);
        let border_color = ratatui::style::Color::Cyan;

        let height = lines.len() as u16 + 4;
        let width = match layout_mode {
            LayoutMode::Compact => self.columns.min(50),
            LayoutMode::Horizontal => self.columns.min(80),
        };

        let x = area.x + (area.width.saturating_sub(width)) / 2;
        let y = area.y;

        let dialog_area = Rect {
            x,
            y,
            width,
            height,
        };

        let border_style = Style::default().fg(border_color);

        for dx in 0..width {
            for dy in 0..height {
                let bx = dialog_area.x + dx;
                let by = dialog_area.y + dy;
                if bx >= buf.area.width || by >= buf.area.height {
                    continue;
                }

                let cell = buf.cell_mut((bx, by)).unwrap();

                if dx == 0 && dy == 0 {
                    cell.set_symbol("┌");
                    cell.set_style(border_style);
                } else if dx == width - 1 && dy == 0 {
                    cell.set_symbol("┐");
                    cell.set_style(border_style);
                } else if dx == 0 && dy == height - 1 {
                    cell.set_symbol("└");
                    cell.set_style(border_style);
                } else if dx == width - 1 && dy == height - 1 {
                    cell.set_symbol("┘");
                    cell.set_style(border_style);
                } else if dx == 0 || dx == width - 1 {
                    cell.set_symbol("│");
                    cell.set_style(border_style);
                } else if dy == 0 || dy == height - 1 {
                    cell.set_symbol("─");
                    cell.set_style(border_style);
                } else if dy == 0 {
                    let title_start = 3;
                    let char_idx = dx as usize;
                    if char_idx >= title_start && char_idx < title_start + border_title.len() {
                        let ch = border_title.chars().nth(char_idx - title_start).unwrap();
                        cell.set_symbol(&ch.to_string());
                        cell.set_style(Style::default().fg(border_color).add_modifier(Modifier::BOLD));
                    } else {
                        cell.set_symbol("─");
                        cell.set_style(border_style);
                    }
                } else {
                    let content_idx = dy - 1;
                    if content_idx < lines.len() as u16 {
                        let line = &lines[content_idx as usize];
                        let mut char_idx = 0u16;
                        for span in &line.spans {
                            for ch in span.content.chars() {
                                if char_idx + 2 < width {
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

impl Widget for LogoHeaderWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}

fn shorten_model(model: &str) -> String {
    if model.is_empty() {
        return "unknown".to_string();
    }
    if model.contains("claude-sonnet-4") {
        return "sonnet 4".to_string();
    }
    if model.contains("claude-opus") {
        return "opus".to_string();
    }
    if model.contains("claude-haiku") {
        return "haiku".to_string();
    }
    if model.contains("claude-") {
        let parts: Vec<&str> = model.splitn(3, '-').collect();
        if parts.len() >= 2 {
            return parts[1].to_string();
        }
    }
    model.to_string()
}

fn truncate_path(path: &str, max_len: usize) -> String {
    if path.len() <= max_len {
        return path.to_string();
    }

    let path_obj = Path::new(path);
    if let Some(file_name) = path_obj.file_name().and_then(|n| n.to_str()) {
        if let Some(parent) = path_obj.parent().and_then(|p| p.to_str()) {
            if parent.is_empty() || parent == "/" {
                return format!("/{file_name}");
            }
            let home = dirs::home_dir()
                .map(|p| p.display().to_string())
                .unwrap_or_default();
            let display_parent = if !home.is_empty() && parent.starts_with(&home) {
                format!("~{}", &parent[home.len()..])
            } else {
                parent.to_string()
            };
            if display_parent.len() + file_name.len() + 3 <= max_len {
                return format!("{}/{}", display_parent, file_name);
            }
            let available = max_len.saturating_sub(file_name.len() + 4);
            if available > 3 {
                return format!("{}/…/{}", &display_parent[..available], file_name);
            }
            return format!("…/{}", file_name);
        }
    }

    format!("…{}", &path[path.len().saturating_sub(max_len - 1)..])
}
