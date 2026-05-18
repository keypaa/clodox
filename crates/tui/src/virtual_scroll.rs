use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::collections::HashMap;

use crate::theme::{Theme, Themeable};
use crate::components::messages::row::{RenderMessage, render_message_row};

const DEFAULT_CAP: usize = 200;
const TRANSCRIPT_CAP: usize = 30;

#[derive(Debug, Clone)]
pub struct VisibleRange {
    pub start_idx: usize,
    pub end_idx: usize,
    pub y_offset: u16,
}

#[derive(Debug, Clone)]
pub struct VirtualMessageList {
    messages: Vec<RenderMessage>,
    viewport_height: u16,
    viewport_width: u16,
    scroll_offset: usize,
    height_cache: HashMap<usize, u16>,
    cumulative_heights: Vec<u16>,
    total_height: u16,
    auto_scroll: bool,
    new_messages_since_scroll: usize,
    message_count_cap: usize,
    transcript_mode: bool,
    cache_valid: bool,
}

impl VirtualMessageList {
    pub fn new() -> Self {
        Self {
            messages: Vec::new(),
            viewport_height: 0,
            viewport_width: 0,
            scroll_offset: 0,
            height_cache: HashMap::new(),
            cumulative_heights: Vec::new(),
            total_height: 0,
            auto_scroll: true,
            new_messages_since_scroll: 0,
            message_count_cap: DEFAULT_CAP,
            transcript_mode: false,
            cache_valid: false,
        }
    }

    pub fn with_transcript_mode(mut self, enabled: bool) -> Self {
        self.transcript_mode = enabled;
        self.message_count_cap = if enabled {
            TRANSCRIPT_CAP
        } else {
            DEFAULT_CAP
        };
        self
    }

    pub fn update_messages(&mut self, messages: Vec<RenderMessage>) {
        let old_len = self.messages.len();
        let capped = if messages.len() > self.message_count_cap {
            messages[messages.len() - self.message_count_cap..].to_vec()
        } else {
            messages
        };

        if capped.len() != old_len {
            self.height_cache.clear();
            self.cache_valid = false;
        }

        let new_count = capped.len().saturating_sub(old_len);
        if new_count > 0 && !self.auto_scroll {
            self.new_messages_since_scroll += new_count;
        }

        self.messages = capped;

        if self.auto_scroll {
            self.scroll_to_bottom();
        }
    }

    pub fn resize(&mut self, width: u16, height: u16) {
        if self.viewport_width != width {
            self.height_cache.clear();
            self.cache_valid = false;
        }
        self.viewport_width = width;
        self.viewport_height = height;
    }

    pub fn get_message_height(&mut self, idx: usize) -> u16 {
        if idx >= self.messages.len() {
            return 0;
        }

        if let Some(&height) = self.height_cache.get(&idx) {
            return height;
        }

        let height = estimate_message_height(&self.messages[idx], self.viewport_width);
        self.height_cache.insert(idx, height);
        height
    }

    fn rebuild_cumulative_heights(&mut self) {
        if self.cache_valid {
            return;
        }

        self.cumulative_heights = Vec::with_capacity(self.messages.len() + 1);
        self.cumulative_heights.push(0);

        let mut total = 0u16;
        for (i, msg) in self.messages.iter().enumerate() {
            let h = self.height_cache.get(&i).copied().unwrap_or_else(|| {
                let h = estimate_message_height(msg, self.viewport_width);
                self.height_cache.insert(i, h);
                h
            });
            total = total.saturating_add(h);
            self.cumulative_heights.push(total);
        }

        self.total_height = total;
        self.cache_valid = true;
    }

    pub fn compute_visible_range(&mut self) -> VisibleRange {
        if self.messages.is_empty() || self.viewport_height == 0 {
            return VisibleRange {
                start_idx: 0,
                end_idx: 0,
                y_offset: 0,
            };
        }

        self.rebuild_cumulative_heights();

        let scroll = self.scroll_offset as u16;
        let viewport_bottom = scroll.saturating_add(self.viewport_height);

        let start_idx = self
            .cumulative_heights
            .iter()
            .position(|&h| h > scroll)
            .unwrap_or(self.messages.len())
            .saturating_sub(1);

        let end_idx = self
            .cumulative_heights
            .iter()
            .position(|&h| h >= viewport_bottom)
            .unwrap_or(self.messages.len());

        let y_offset = scroll.saturating_sub(
            *self
                .cumulative_heights
                .get(start_idx)
                .unwrap_or(&0),
        );

        VisibleRange {
            start_idx,
            end_idx: end_idx.min(self.messages.len()),
            y_offset,
        }
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.auto_scroll = false;
        let max_scroll = self.total_height.saturating_sub(self.viewport_height) as usize;
        self.scroll_offset = self
            .scroll_offset
            .saturating_add(lines as usize)
            .min(max_scroll);
    }

    pub fn scroll_down(&mut self, lines: u16) {
        let max_scroll = self.total_height.saturating_sub(self.viewport_height) as usize;
        self.scroll_offset = self
            .scroll_offset
            .saturating_sub(lines as usize)
            .max(0);

        if self.scroll_offset >= max_scroll.saturating_sub(1) {
            self.auto_scroll = true;
            self.new_messages_since_scroll = 0;
        }
    }

    pub fn scroll_to_bottom(&mut self) {
        self.auto_scroll = true;
        self.new_messages_since_scroll = 0;
        self.rebuild_cumulative_heights();
        let max_scroll = self.total_height.saturating_sub(self.viewport_height);
        self.scroll_offset = max_scroll as usize;
    }

    pub fn is_at_bottom(&self) -> bool {
        if self.total_height <= self.viewport_height {
            return true;
        }
        let max_scroll = self.total_height.saturating_sub(self.viewport_height) as usize;
        self.scroll_offset >= max_scroll.saturating_sub(1)
    }

    pub fn new_messages_count(&self) -> usize {
        if self.auto_scroll {
            0
        } else {
            self.new_messages_since_scroll
        }
    }

    pub fn message_count(&self) -> usize {
        self.messages.len()
    }

    pub fn total_content_height(&self) -> u16 {
        self.total_height
    }

    pub fn scroll_offset(&self) -> usize {
        self.scroll_offset
    }

    pub fn is_transcript_mode(&self) -> bool {
        self.transcript_mode
    }

    pub fn set_transcript_mode(&mut self, enabled: bool) {
        if self.transcript_mode != enabled {
            self.transcript_mode = enabled;
            self.message_count_cap = if enabled {
                TRANSCRIPT_CAP
            } else {
                DEFAULT_CAP
            };
            self.height_cache.clear();
            self.cache_valid = false;
        }
    }
}

impl Default for VirtualMessageList {
    fn default() -> Self {
        Self::new()
    }
}

fn estimate_message_height(message: &RenderMessage, width: u16) -> u16 {
    let content_width = width.saturating_sub(2);
    if content_width == 0 {
        return 1;
    }

    match message {
        RenderMessage::UserText { text } => {
            if text.is_empty() {
                return 1;
            }
            let line_count = text.lines().count();
            let mut wrapped = 0u16;
            for line in text.lines() {
                let chars = unicode_width::UnicodeWidthStr::width(line);
                wrapped += (chars as u16).div_ceil(content_width);
            }
            (line_count as u16).max(wrapped).max(1)
        }
        RenderMessage::UserPrompt { content } => {
            if content.is_empty() {
                return 1;
            }
            let line_count = content.lines().count();
            let mut wrapped = 0u16;
            for line in content.lines() {
                let chars = unicode_width::UnicodeWidthStr::width(line);
                wrapped += (chars as u16).div_ceil(content_width);
            }
            (line_count as u16).max(wrapped).max(1)
        }
        RenderMessage::UserCommand { .. } => 1,
        RenderMessage::UserToolResult { content, .. } => {
            if content.is_empty() {
                return 1;
            }
            let line_count = content.lines().count();
            let mut wrapped = 0u16;
            for line in content.lines() {
                let chars = unicode_width::UnicodeWidthStr::width(line);
                wrapped += (chars as u16).div_ceil(content_width);
            }
            (line_count as u16).max(wrapped).max(1)
        }
        RenderMessage::AssistantText { text } => {
            if text.is_empty() {
                return 1;
            }
            let line_count = text.lines().count();
            let mut wrapped = 0u16;
            for line in text.lines() {
                let chars = unicode_width::UnicodeWidthStr::width(line);
                wrapped += (chars as u16).div_ceil(content_width);
            }
            (line_count as u16).max(wrapped).max(1)
        }
        RenderMessage::AssistantToolUse {
            details,
            status,
            ..
        } => {
            let mut h = 1;
            if let Some(d) = details {
                let chars = unicode_width::UnicodeWidthStr::width(d.as_str());
                h += (chars as u16).div_ceil(content_width);
            }
            if status.is_some() {
                h += 1;
            }
            h.max(1)
        }
        RenderMessage::AssistantThinking { is_expanded, thinking } => {
            if *is_expanded {
                if thinking.is_empty() {
                    return 2;
                }
                let line_count = thinking.lines().count();
                let mut wrapped = 0u16;
                for line in thinking.lines() {
                    let chars = unicode_width::UnicodeWidthStr::width(line);
                    wrapped += (chars as u16).div_ceil(content_width);
                }
                1 + (line_count as u16).max(wrapped)
            } else {
                1
            }
        }
        RenderMessage::SystemError { error } => {
            if error.is_empty() {
                return 1;
            }
            let line_count = error.lines().count();
            let mut wrapped = 0u16;
            for line in error.lines() {
                let chars = unicode_width::UnicodeWidthStr::width(line);
                wrapped += (chars as u16).div_ceil(content_width);
            }
            let h = (line_count as u16).max(wrapped).min(10);
            if error.len() > 1000 {
                h + 1
            } else {
                h.max(1)
            }
        }
        RenderMessage::RateLimit {
            text,
            upgrade_hint,
        } => {
            let mut h = 1;
            if let Some(hint) = upgrade_hint {
                let chars = unicode_width::UnicodeWidthStr::width(hint.as_str());
                h += (chars as u16).div_ceil(content_width);
            }
            h.max(1)
        }
    }
}

pub struct NewMessagesPill {
    count: usize,
}

impl NewMessagesPill {
    pub fn new(count: usize) -> Self {
        Self { count }
    }
}

impl Themeable for NewMessagesPill {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        if self.count == 0 {
            return;
        }

        let text = if self.count == 1 {
            "↓ 1 new message".to_string()
        } else {
            format!("↓ {} new messages", self.count)
        };

        let pill_width = text.len() as u16 + 4;
        let pill_x = area.x + (area.width.saturating_sub(pill_width)) / 2;
        let pill_y = area.y;

        if pill_y >= buf.area.height {
            return;
        }

        let pill_style = Style::default()
            .fg(theme.colors.suggestion)
            .bg(theme.colors.message_actions_background)
            .add_modifier(Modifier::BOLD);

        let border_style = Style::default()
            .fg(theme.colors.suggestion)
            .bg(theme.colors.message_actions_background);

        for dx in 0..pill_width.min(area.width) {
            let bx = pill_x + dx;
            if bx >= buf.area.width {
                continue;
            }

            let cell = buf.cell_mut((bx, pill_y)).unwrap();

            if dx == 0 {
                cell.set_symbol("╭");
                cell.set_style(border_style);
            } else if dx == pill_width - 1 {
                cell.set_symbol("╮");
                cell.set_style(border_style);
            } else {
                let char_idx = dx - 1;
                if char_idx < text.len() as u16 {
                    if let Some(ch) = text.chars().nth(char_idx as usize) {
                        cell.set_symbol(&ch.to_string());
                        cell.set_style(pill_style);
                    }
                } else {
                    cell.set_symbol(" ");
                    cell.set_style(border_style);
                }
            }
        }

        for dx in 0..pill_width.min(area.width) {
            let bx = pill_x + dx;
            let by = pill_y + 1;
            if bx >= buf.area.width || by >= buf.area.height {
                continue;
            }

            let cell = buf.cell_mut((bx, by)).unwrap();

            if dx == 0 {
                cell.set_symbol("╰");
                cell.set_style(border_style);
            } else if dx == pill_width - 1 {
                cell.set_symbol("╯");
                cell.set_style(border_style);
            } else {
                cell.set_symbol(" ");
                cell.set_style(border_style);
            }
        }
    }
}

impl Widget for NewMessagesPill {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}

pub struct VirtualMessageListWidget<'a> {
    list: &'a mut VirtualMessageList,
    theme: &'a Theme,
}

impl<'a> VirtualMessageListWidget<'a> {
    pub fn new(list: &'a mut VirtualMessageList, theme: &'a Theme) -> Self {
        Self { list, theme }
    }
}

impl Themeable for VirtualMessageListWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
        let _list = &mut *unsafe { &mut *(self.list as *const VirtualMessageList as *mut VirtualMessageList) };
        let range = _list.compute_visible_range();

        if range.start_idx >= range.end_idx {
            return;
        }

        let mut y = area.y.saturating_sub(range.y_offset);
        let base_y = area.y;

        for idx in range.start_idx..range.end_idx {
            if y >= area.bottom() {
                break;
            }

            let height = _list.get_message_height(idx);
            if height == 0 {
                continue;
            }

            let render_y = base_y + (y as i16).max(0) as u16;
            if render_y >= buf.area.height {
                y += height;
                continue;
            }

            let render_area = Rect {
                x: area.x,
                y: render_y,
                width: area.width,
                height: height.min(area.bottom().saturating_sub(render_y)),
            };

            let msg = &_list.messages[idx];
            render_message_to_buffer(buf, render_area, msg, theme, false);

            y += height;
        }
    }
}

impl Widget for VirtualMessageListWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}

fn render_message_to_buffer(
    buf: &mut ratatui::buffer::Buffer,
    area: Rect,
    message: &RenderMessage,
    theme: &Theme,
    is_selected: bool,
) {
    let lines = message_to_lines(message, area.width, theme, is_selected);

    let y_end = area.y.min(buf.area.height.saturating_sub(1))
        + area.height.min(buf.area.height.saturating_sub(area.y));

    for (i, line) in lines.iter().enumerate() {
        let y = area.y + i as u16;
        if y >= area.bottom() || y >= buf.area.height {
            break;
        }

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

fn message_to_lines(
    message: &RenderMessage,
    width: u16,
    theme: &Theme,
    is_selected: bool,
) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let dot_char = "●";
    let dot_color = if is_selected {
        ratatui::style::Color::Blue
    } else {
        theme.colors.text
    };
    let dot_style = Style::default().fg(dot_color);
    let text_style = Style::default().fg(theme.colors.text);
    let dim_style = Style::default()
        .fg(theme.colors.inactive)
        .add_modifier(Modifier::DIM);
    let bold_style = Style::default()
        .fg(theme.colors.text)
        .add_modifier(Modifier::BOLD);
    let error_style = Style::default().fg(theme.colors.error);
    let success_style = Style::default().fg(theme.colors.success);
    let content_width = width.saturating_sub(2);

    let dot_line = Line::from(vec![
        Span::styled(dot_char, dot_style),
        Span::raw(" "),
    ]);

    match message {
        RenderMessage::UserText { text } => {
            let pointer_style = Style::default().fg(theme.colors.suggestion);
            let pointer_line = Line::from(vec![
                Span::styled("› ", pointer_style),
            ]);
            lines.push(pointer_line);
            if content_width > 0 {
                for line in text.lines() {
                    let wrapped = wrap_line(line, content_width);
                    for w in wrapped {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(w, text_style),
                        ]));
                    }
                    if line.is_empty() {
                        lines.push(Line::from(vec![Span::raw("  ")]));
                    }
                }
            } else {
                lines.push(Line::from(vec![Span::styled(text.clone(), text_style)]));
            }
        }
        RenderMessage::UserPrompt { content } => {
            let pointer_style = Style::default().fg(theme.colors.suggestion);
            lines.push(Line::from(vec![
                Span::styled("› ", pointer_style),
            ]));
            if content_width > 0 {
                for line in content.lines() {
                    let wrapped = wrap_line(line, content_width);
                    for w in wrapped {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(w, text_style),
                        ]));
                    }
                }
            } else {
                lines.push(Line::from(vec![Span::styled(content.clone(), text_style)]));
            }
        }
        RenderMessage::UserCommand { command, args } => {
            let cmd_style = Style::default()
                .fg(theme.colors.suggestion)
                .add_modifier(Modifier::BOLD);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(format!("/{}", command), cmd_style),
                Span::styled(format!(" {}", args), text_style),
            ]));
        }
        RenderMessage::UserToolResult { content, is_error } => {
            if *is_error {
                let display = if content.len() > 1000 {
                    format!("{}…", &content[..999])
                } else {
                    content.clone()
                };
                if content_width > 0 {
                    for line in display.lines() {
                        let wrapped = wrap_line(line, content_width);
                        for w in wrapped {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(w, error_style),
                            ]));
                        }
                    }
                } else {
                    lines.push(Line::from(vec![Span::styled(display, error_style)]));
                }
            } else {
                if content_width > 0 {
                    for line in content.lines() {
                        let wrapped = wrap_line(line, content_width);
                        for w in wrapped {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(w, text_style),
                            ]));
                        }
                    }
                } else {
                    lines.push(Line::from(vec![Span::styled(content.clone(), text_style)]));
                }
            }
        }
        RenderMessage::AssistantText { text } => {
            lines.push(dot_line.clone());
            if content_width > 0 {
                for line in text.lines() {
                    let wrapped = wrap_line(line, content_width);
                    for w in wrapped {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(w, text_style),
                        ]));
                    }
                }
            } else {
                lines.push(Line::from(vec![Span::styled(text.clone(), text_style)]));
            }
        }
        RenderMessage::AssistantToolUse {
            tool_name,
            details,
            status,
            is_resolved,
            is_error,
        } => {
            let loader_color = if *is_error {
                theme.colors.error
            } else if *is_resolved {
                theme.colors.success
            } else {
                theme.colors.inactive
            };
            let loader_style = Style::default().fg(loader_color);

            let mut spans = vec![
                Span::styled(dot_char, loader_style),
                Span::raw(" "),
                Span::styled(tool_name.clone(), bold_style),
            ];

            if let Some(d) = details {
                spans.push(Span::styled(format!(" ({})", d), text_style));
            }
            lines.push(Line::from(spans));

            if let Some(s) = status {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(s.clone(), dim_style),
                ]));
            }
        }
        RenderMessage::AssistantThinking {
            thinking,
            is_expanded,
        } => {
            let thinking_style = Style::default()
                .fg(theme.colors.inactive)
                .add_modifier(Modifier::ITALIC);
            if *is_expanded {
                lines.push(Line::from(vec![
                    Span::styled("∴ Thinking…", thinking_style),
                ]));
                if content_width > 0 {
                    for line in thinking.lines() {
                        let wrapped = wrap_line(line, content_width);
                        for w in wrapped {
                            lines.push(Line::from(vec![
                                Span::raw("  "),
                                Span::styled(w, thinking_style),
                            ]));
                        }
                    }
                } else {
                    lines.push(Line::from(vec![Span::styled(thinking.clone(), thinking_style)]));
                }
            } else {
                lines.push(Line::from(vec![
                    Span::styled("∴ Thinking (ctrl+o to expand)", thinking_style),
                ]));
            }
        }
        RenderMessage::SystemError { error } => {
            let display = if error.len() > 1000 {
                format!("{}…", &error[..999])
            } else {
                error.clone()
            };
            if content_width > 0 {
                for line in display.lines() {
                    let wrapped = wrap_line(line, content_width);
                    for w in wrapped {
                        lines.push(Line::from(vec![
                            Span::raw("  "),
                            Span::styled(w, error_style),
                        ]));
                    }
                }
            } else {
                lines.push(Line::from(vec![Span::styled(display, error_style)]));
            }
            if error.len() > 1000 {
                lines.push(Line::from(vec![
                    Span::styled("(ctrl+o to see all)", dim_style),
                ]));
            }
        }
        RenderMessage::RateLimit {
            text,
            upgrade_hint,
        } => {
            let warning_style = Style::default()
                .fg(theme.colors.warning)
                .add_modifier(Modifier::BOLD);
            lines.push(Line::from(vec![
                Span::raw("  "),
                Span::styled(text.clone(), warning_style),
            ]));
            if let Some(hint) = upgrade_hint {
                lines.push(Line::from(vec![
                    Span::raw("  "),
                    Span::styled(hint.clone(), dim_style),
                ]));
            }
        }
    }

    if lines.is_empty() {
        lines.push(Line::from(vec![Span::raw("")]));
    }

    lines
}

fn wrap_line(line: &str, max_width: u16) -> Vec<String> {
    if max_width == 0 {
        return vec![line.to_string()];
    }

    let mut result = Vec::new();
    let mut current = String::new();
    let mut current_width = 0;

    for ch in line.chars() {
        let ch_width = unicode_width::UnicodeWidthChar::width(ch).unwrap_or(1) as u16;
        if current_width + ch_width > max_width && !current.is_empty() {
            result.push(current);
            current = String::new();
            current_width = 0;
        }
        current.push(ch);
        current_width += ch_width;
    }

    if !current.is_empty() {
        result.push(current);
    }

    if result.is_empty() {
        result.push(String::new());
    }

    result
}
