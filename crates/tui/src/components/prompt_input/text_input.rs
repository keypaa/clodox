use ratatui::layout::Rect;
use ratatui::style::Style;
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

use crate::theme::{Theme, Themeable};
use crate::input::{InputAction, KillRing};

const POINTER_CHAR: &str = "›";
const MIN_VISIBLE_LINES: usize = 3;

#[derive(Debug, Clone)]
pub struct TextInput {
    lines: Vec<String>,
    cursor_line: usize,
    cursor_col: usize,
    kill_ring: KillRing,
    submitted_text: Vec<String>,
    max_lines: usize,
    show_footer_hint: bool,
    escape_confirmation: Option<bool>,
}

impl TextInput {
    pub fn new() -> Self {
        Self {
            lines: vec![String::new()],
            cursor_line: 0,
            cursor_col: 0,
            kill_ring: KillRing::new(),
            submitted_text: Vec::new(),
            max_lines: 20,
            show_footer_hint: false,
            escape_confirmation: None,
        }
    }

    pub fn text(&self) -> String {
        self.lines.join("\n")
    }

    pub fn is_empty(&self) -> bool {
        self.lines.len() == 1 && self.lines[0].is_empty()
    }

    pub fn cursor_position(&self) -> (usize, usize) {
        (self.cursor_line, self.cursor_col)
    }

    pub fn line_count(&self) -> usize {
        self.lines.len()
    }

    pub fn handle_action(&mut self, action: InputAction) -> Option<String> {
        match action {
            InputAction::InsertChar(c) => self.insert_char(c),
            InputAction::InsertNewline => self.insert_newline(),
            InputAction::Backspace => self.backspace(),
            InputAction::Delete => self.delete(),
            InputAction::MoveLeft => self.move_left(),
            InputAction::MoveRight => self.move_right(),
            InputAction::MoveUp => self.move_up(),
            InputAction::MoveDown => self.move_down(),
            InputAction::MoveStartOfLine => self.move_start_of_line(),
            InputAction::MoveEndOfLine => self.move_end_of_line(),
            InputAction::MovePrevWord => self.move_prev_word(),
            InputAction::MoveNextWord => self.move_next_word(),
            InputAction::KillToLineEnd => self.kill_to_line_end(),
            InputAction::KillToLineStart => self.kill_to_line_start(),
            InputAction::KillWordBefore => self.kill_word_before(),
            InputAction::Yank => self.yank(),
            InputAction::Cancel => self.cancel(),
            InputAction::Submit => return self.submit(),
            _ => {}
        }
        None
    }

    fn insert_char(&mut self, c: char) {
        let line = &mut self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let mut new_line = String::new();
        for (i, g) in graphemes.iter().enumerate() {
            if i == self.cursor_col {
                new_line.push(c);
            }
            new_line.push_str(g);
        }
        if self.cursor_col >= graphemes.len() {
            new_line.push(c);
        }
        self.lines[self.cursor_line] = new_line;
        self.cursor_col += 1;
        self.escape_confirmation = None;
    }

    fn insert_newline(&mut self) {
        if self.lines.len() >= self.max_lines {
            return;
        }
        let line = &mut self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let after = graphemes[self.cursor_col..].join("");
        self.lines[self.cursor_line] = graphemes[..self.cursor_col].join("");
        self.lines.insert(self.cursor_line + 1, after);
        self.cursor_line += 1;
        self.cursor_col = 0;
    }

    fn backspace(&mut self) {
        if self.cursor_col > 0 {
            let line = &mut self.lines[self.cursor_line];
            let graphemes: Vec<&str> = line.graphemes(true).collect();
            if self.cursor_col <= graphemes.len() {
                let mut new_line = String::new();
                for (i, g) in graphemes.iter().enumerate() {
                    if i != self.cursor_col - 1 {
                        new_line.push_str(g);
                    }
                }
                self.lines[self.cursor_line] = new_line;
                self.cursor_col -= 1;
            }
        } else if self.cursor_line > 0 {
            let current_line = self.lines.remove(self.cursor_line);
            self.cursor_line -= 1;
            let prev_line_len = self.lines[self.cursor_line].graphemes(true).count();
            self.lines[self.cursor_line].push_str(&current_line);
            self.cursor_col = prev_line_len;
        }
    }

    fn delete(&mut self) {
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        if self.cursor_col < graphemes.len() {
            let mut new_line = String::new();
            for (i, g) in graphemes.iter().enumerate() {
                if i != self.cursor_col {
                    new_line.push_str(g);
                }
            }
            self.lines[self.cursor_line] = new_line;
        } else if self.cursor_line < self.lines.len() - 1 {
            let next_line = self.lines.remove(self.cursor_line + 1);
            self.lines[self.cursor_line].push_str(&next_line);
        }
    }

    fn move_left(&mut self) {
        if self.cursor_col > 0 {
            let line = &self.lines[self.cursor_line];
            let graphemes: Vec<&str> = line.graphemes(true).collect();
            if self.cursor_col <= graphemes.len() {
                let before = &graphemes[..self.cursor_col - 1];
                self.cursor_col = before.len();
            }
        }
    }

    fn move_right(&mut self) {
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        if self.cursor_col < graphemes.len() {
            self.cursor_col += 1;
        }
    }

    fn move_up(&mut self) {
        if self.cursor_line > 0 {
            let current_line = &self.lines[self.cursor_line];
            let current_col_width = current_line.graphemes(true).take(self.cursor_col).map(|g| g.width()).sum::<usize>();
            self.cursor_line -= 1;
            let prev_line = &self.lines[self.cursor_line];
            let graphemes: Vec<&str> = prev_line.graphemes(true).collect();
            let mut col = 0;
            let mut width = 0;
            for (i, g) in graphemes.iter().enumerate() {
                if width + g.width() > current_col_width {
                    break;
                }
                width += g.width();
                col = i + 1;
            }
            self.cursor_col = col;
        }
    }

    fn move_down(&mut self) {
        if self.cursor_line < self.lines.len() - 1 {
            let current_line = &self.lines[self.cursor_line];
            let current_col_width = current_line.graphemes(true).take(self.cursor_col).map(|g| g.width()).sum::<usize>();
            self.cursor_line += 1;
            let next_line = &self.lines[self.cursor_line];
            let graphemes: Vec<&str> = next_line.graphemes(true).collect();
            let mut col = 0;
            let mut width = 0;
            for (i, g) in graphemes.iter().enumerate() {
                if width + g.width() > current_col_width {
                    break;
                }
                width += g.width();
                col = i + 1;
            }
            self.cursor_col = col;
        }
    }

    fn move_start_of_line(&mut self) {
        self.cursor_col = 0;
    }

    fn move_end_of_line(&mut self) {
        let line = &self.lines[self.cursor_line];
        self.cursor_col = line.graphemes(true).count();
    }

    fn move_prev_word(&mut self) {
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let before: String = graphemes[..self.cursor_col].join("");
        let chars: Vec<char> = before.chars().collect();
        let mut i = chars.len();
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        self.cursor_col = before[..i].graphemes(true).count();
    }

    fn move_next_word(&mut self) {
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let after: String = graphemes[self.cursor_col..].join("");
        let chars: Vec<char> = after.chars().collect();
        let mut i = 0;
        while i < chars.len() && chars[i].is_whitespace() {
            i += 1;
        }
        while i < chars.len() && !chars[i].is_whitespace() {
            i += 1;
        }
        self.cursor_col += after[..i].graphemes(true).count();
    }

    fn kill_to_line_end(&mut self) {
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        if self.cursor_col < graphemes.len() {
            let killed = graphemes[self.cursor_col..].join("");
            self.kill_ring.push(killed);
            self.lines[self.cursor_line] = graphemes[..self.cursor_col].join("");
        }
    }

    fn kill_to_line_start(&mut self) {
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        if self.cursor_col > 0 {
            let killed = graphemes[..self.cursor_col].join("");
            self.kill_ring.push(killed);
            self.lines[self.cursor_line] = graphemes[self.cursor_col..].join("");
            self.cursor_col = 0;
        }
    }

    fn kill_word_before(&mut self) {
        let line = &self.lines[self.cursor_line];
        let graphemes: Vec<&str> = line.graphemes(true).collect();
        let before: String = graphemes[..self.cursor_col].join("");
        let chars: Vec<char> = before.chars().collect();
        let mut i = chars.len();
        while i > 0 && chars[i - 1].is_whitespace() {
            i -= 1;
        }
        while i > 0 && !chars[i - 1].is_whitespace() {
            i -= 1;
        }
        let killed = before[i..].to_string();
        if !killed.is_empty() {
            self.kill_ring.push(killed);
            let remaining = before[..i].to_string();
            let after: String = graphemes[self.cursor_col..].join("");
            self.lines[self.cursor_line] = format!("{}{}", remaining, after);
            self.cursor_col = before[..i].graphemes(true).count();
        }
    }

    fn yank(&mut self) {
        if let Some(text) = self.kill_ring.current().map(|s| s.to_string()) {
            let line = &mut self.lines[self.cursor_line];
            let graphemes: Vec<&str> = line.graphemes(true).collect();
            let before = graphemes[..self.cursor_col].join("");
            let after = graphemes[self.cursor_col..].join("");
            self.lines[self.cursor_line] = format!("{}{}{}", before, text, after);
            self.cursor_col += text.graphemes(true).count();
        }
    }

    fn submit(&mut self) -> Option<String> {
        let text = self.text();
        if !text.trim().is_empty() {
            self.submitted_text.push(text.clone());
            self.lines = vec![String::new()];
            self.cursor_line = 0;
            self.cursor_col = 0;
            Some(text)
        } else {
            None
        }
    }

    fn cancel(&mut self) {
        if self.escape_confirmation.is_some() {
            self.lines = vec![String::new()];
            self.cursor_line = 0;
            self.cursor_col = 0;
            self.escape_confirmation = None;
        } else {
            self.escape_confirmation = Some(true);
        }
    }

    pub fn get_kill_ring(&self) -> &KillRing {
        &self.kill_ring
    }

    pub fn get_kill_ring_mut(&mut self) -> &mut KillRing {
        &mut self.kill_ring
    }

    pub fn escape_confirmation(&self) -> bool {
        self.escape_confirmation.is_some()
    }

    pub fn clear_escape_confirmation(&mut self) {
        self.escape_confirmation = None;
    }
}

impl Default for TextInput {
    fn default() -> Self {
        Self::new()
    }
}

pub struct TextInputWidget<'a> {
    input: &'a TextInput,
    theme: &'a Theme,
    width: u16,
    max_visible_lines: usize,
}

impl<'a> TextInputWidget<'a> {
    pub fn new(input: &'a TextInput, theme: &'a Theme, width: u16) -> Self {
        Self {
            input,
            theme,
            width,
            max_visible_lines: MIN_VISIBLE_LINES,
        }
    }

    pub fn with_max_lines(mut self, max: usize) -> Self {
        self.max_visible_lines = max;
        self
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        let mut result = Vec::new();
        let max_lines = self.max_visible_lines.min(self.input.lines.len());
        let start_line = if self.input.lines.len() > max_lines {
            self.input.lines.len() - max_lines
        } else {
            0
        };

        for line_idx in start_line..self.input.lines.len() {
            let line_text = &self.input.lines[line_idx];
            let graphemes: Vec<&str> = line_text.graphemes(true).collect();

            if line_idx == self.input.cursor_line {
                let mut spans = vec![
                    Span::styled(
                        format!("{} ", POINTER_CHAR),
                        Style::default().fg(self.theme.colors.suggestion),
                    ),
                ];

                let before: String = graphemes[..self.input.cursor_col.min(graphemes.len())].join("");
                spans.push(Span::styled(
                    before,
                    Style::default().fg(self.theme.colors.text),
                ));

                if self.input.cursor_col < graphemes.len() {
                    let cursor_char = graphemes[self.input.cursor_col];
                    spans.push(Span::styled(
                        cursor_char.to_string(),
                        Style::default()
                            .bg(self.theme.colors.text)
                            .fg(self.theme.colors.user_message_background),
                    ));
                    let after: String = graphemes[self.input.cursor_col + 1..].join("");
                    if !after.is_empty() {
                        spans.push(Span::styled(
                            after,
                            Style::default().fg(self.theme.colors.text),
                        ));
                    }
                } else {
                    spans.push(Span::styled(
                        " ",
                        Style::default()
                            .bg(self.theme.colors.text)
                            .fg(self.theme.colors.user_message_background),
                    ));
                }

                result.push(Line::from(spans));
            } else {
                let spans = vec![
                    Span::raw("  "),
                    Span::styled(
                        line_text.clone(),
                        Style::default().fg(self.theme.colors.text),
                    ),
                ];
                result.push(Line::from(spans));
            }
        }

        result
    }
}

impl Themeable for TextInputWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, _theme: &Theme) {
        let lines = self.render_lines();
        let y_end = (area.y + area.height).min(buf.area.height);
        for (i, line) in lines.iter().enumerate() {
            let y = area.y + i as u16;
            if y >= area.y && y < y_end {
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
    }
}

impl Widget for TextInputWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
