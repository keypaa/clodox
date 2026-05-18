use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::time::{Duration, Instant};

use crate::theme::{Theme, Themeable};
use crate::state::AppState;

const DEBOUNCE_INTERVAL: Duration = Duration::from_millis(300);

#[derive(Debug, Clone)]
pub struct StatusLine {
    last_update: Instant,
    cached_spans: Vec<Span<'static>>,
    cached_width: u16,
    needs_rebuild: bool,
}

impl StatusLine {
    pub fn new() -> Self {
        Self {
            last_update: Instant::now(),
            cached_spans: Vec::new(),
            cached_width: 0,
            needs_rebuild: true,
        }
    }

    pub fn update(&mut self, state: &AppState, width: u16) {
        let now = Instant::now();
        if !self.needs_rebuild
            && now.duration_since(self.last_update) < DEBOUNCE_INTERVAL
            && self.cached_width == width
        {
            return;
        }

        self.cached_spans = build_status_spans(state);
        self.cached_width = width;
        self.last_update = now;
        self.needs_rebuild = false;
    }

    pub fn mark_dirty(&mut self) {
        self.needs_rebuild = true;
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        if self.cached_spans.is_empty() {
            return vec![];
        }
        vec![Line::from(self.cached_spans.clone())]
    }
}

impl Default for StatusLine {
    fn default() -> Self {
        Self::new()
    }
}

fn build_status_spans(state: &AppState) -> Vec<Span<'static>> {
    let mut spans = Vec::new();

    let model_short = shorten_model(&state.main_loop_model.name);
    spans.push(Span::styled(
        model_short,
        Style::default().fg(ratatui::style::Color::Cyan),
    ));

    if state.fast_mode {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "fast",
            Style::default()
                .fg(ratatui::style::Color::Yellow)
                .add_modifier(Modifier::DIM),
        ));
    }

    if state.thinking_enabled {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "think",
            Style::default()
                .fg(ratatui::style::Color::Magenta)
                .add_modifier(Modifier::DIM),
        ));
    }

    match state.effort {
        cc_core::types::EffortValue::High => {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "high",
                Style::default()
                    .fg(ratatui::style::Color::Red)
                    .add_modifier(Modifier::DIM),
            ));
        }
        cc_core::types::EffortValue::Low => {
            spans.push(Span::raw(" "));
            spans.push(Span::styled(
                "low",
                Style::default()
                    .fg(ratatui::style::Color::DarkGray)
                    .add_modifier(Modifier::DIM),
            ));
        }
        _ => {}
    }

    let perm_label = permission_mode_label(&state.tool_permission_context.mode);
    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        perm_label,
        Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));

    if !state.tool_permission_context.additional_working_directories.is_empty() {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "dirs",
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ));
    }

    spans.push(Span::raw(" "));
    spans.push(Span::styled(
        "$0.00",
        Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM),
    ));

    if state.total_cost_usd > 0.0 {
        spans.push(Span::styled(
            format!("${:.2}", state.total_cost_usd),
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ));
    }

    let total_input = state.token_counts.input_tokens + state.token_counts.cache_read_tokens;
    let total_output = state.token_counts.output_tokens;
    if total_input > 0 || total_output > 0 {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            format_tokens(total_input, total_output),
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ));
    }

    if state.is_querying {
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            "●",
            Style::default().fg(ratatui::style::Color::Green),
        ));
    }

    if let Some(ref session_id) = state.session_id {
        let short_id = if session_id.len() > 8 {
            format!("...{}", &session_id[session_id.len() - 8..])
        } else {
            session_id.clone()
        };
        spans.push(Span::raw(" "));
        spans.push(Span::styled(
            short_id,
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM),
        ));
    }

    spans
}

fn shorten_model(model: &str) -> String {
    if model.is_empty() {
        return "unknown".to_string();
    }

    if model.contains("claude-sonnet-4") {
        return "sonnet".to_string();
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

fn permission_mode_label(mode: &cc_core::permissions::PermissionMode) -> String {
    match mode {
        cc_core::permissions::PermissionMode::Default => "default".to_string(),
        cc_core::permissions::PermissionMode::AcceptEdits => "accept-edits".to_string(),
        cc_core::permissions::PermissionMode::BypassPermissions => "bypass".to_string(),
        cc_core::permissions::PermissionMode::DontAsk => "dont-ask".to_string(),
        cc_core::permissions::PermissionMode::Plan => "plan".to_string(),
        cc_core::permissions::PermissionMode::Auto => "auto".to_string(),
        cc_core::permissions::PermissionMode::Bubble => "bubble".to_string(),
    }
}

fn format_tokens(input: u64, output: u64) -> String {
    if input == 0 && output == 0 {
        return String::new();
    }

    if input == 0 {
        return format!("↑{}", format_number(output));
    }
    if output == 0 {
        return format!("↓{}", format_number(input));
    }

    format!("↓{} ↑{}", format_number(input), format_number(output))
}

fn format_number(n: u64) -> String {
    if n >= 1_000_000 {
        format!("{:.1}M", n as f64 / 1_000_000.0)
    } else if n >= 1_000 {
        format!("{:.1}k", n as f64 / 1_000.0)
    } else {
        n.to_string()
    }
}

pub struct StatusLineWidget<'a> {
    status: &'a StatusLine,
    theme: &'a Theme,
}

impl<'a> StatusLineWidget<'a> {
    pub fn new(status: &'a StatusLine, theme: &'a Theme) -> Self {
        Self { status, theme }
    }
}

impl Themeable for StatusLineWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, _theme: &Theme) {
        let lines = self.status.render_lines();
        if lines.is_empty() {
            return;
        }

        let line = &lines[0];
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

impl Widget for StatusLineWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
