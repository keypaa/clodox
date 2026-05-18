use std::io;
use std::time::Duration;

use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};

use crate::input::{InputAction, InputHandler};
use crate::state::{create_state, write_state, SharedState};
use crate::terminal::TerminalManager;
use crate::theme::Theme;

/// TUI main loop.
pub struct TuiApp {
    pub terminal: TerminalManager,
    pub state: SharedState,
    pub input_handler: InputHandler,
    pub theme: Theme,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub messages: Vec<String>,
    pub is_running: bool,
}

impl TuiApp {
    pub fn new() -> io::Result<Self> {
        let terminal = TerminalManager::new()?;
        let state = create_state();
        let input_handler = InputHandler::new();
        let theme = Theme::default();

        Ok(Self {
            terminal,
            state,
            input_handler,
            theme,
            input_buffer: String::new(),
            cursor_position: 0,
            messages: vec![
                "Welcome to Claude Code (Rust port)".to_string(),
                "Type a message and press Enter to send.".to_string(),
                "Press Ctrl+C twice to exit.".to_string(),
                "".to_string(),
            ],
            is_running: true,
        })
    }

    /// Run the main TUI loop.
    pub fn run(&mut self) -> io::Result<()> {
        self.terminal.initialize()?;

        while self.is_running {
            // Poll for events with a short timeout (for animation ticks)
            let has_event = self.terminal.poll_event(Duration::from_millis(50))?;
            if has_event {
                let event = self.terminal.read_event()?;
                // Process event without holding any locks
                let action = self.input_handler.process_event(event);
                if let Some(action) = action {
                    self.handle_action(action);
                }
            }

            // Snapshot all state before rendering
            let messages = self.messages.clone();
            let input_buffer = self.input_buffer.clone();
            let cursor_position = self.cursor_position;
            let is_querying = self
                .state
                .read()
                .map(|s| s.is_querying)
                .unwrap_or(false);

            // Render with snapshot data only
            self.terminal.draw(|frame| {
                Self::render_frame_static(
                    frame,
                    &messages,
                    &input_buffer,
                    cursor_position,
                    is_querying,
                );
            })?;
        }

        self.terminal.cleanup()?;
        Ok(())
    }

    /// Handle a crossterm event.
    fn handle_event(&mut self, event: Event) {
        let action = self.input_handler.process_event(event);

        if let Some(action) = action {
            self.handle_action(action);
        }
    }

    /// Handle an input action.
    fn handle_action(&mut self, action: InputAction) {
        match action {
            InputAction::InsertChar(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
            }
            InputAction::InsertNewline => {
                self.input_buffer.insert(self.cursor_position, '\n');
                self.cursor_position += 1;
            }
            InputAction::Backspace => {
                if self.cursor_position > 0 {
                    self.input_buffer.remove(self.cursor_position - 1);
                    self.cursor_position -= 1;
                }
            }
            InputAction::Delete => {
                if self.cursor_position < self.input_buffer.len() {
                    self.input_buffer.remove(self.cursor_position);
                }
            }
            InputAction::KillToLineEnd => {
                self.input_buffer.truncate(self.cursor_position);
            }
            InputAction::KillToLineStart => {
                let killed = self.input_buffer.drain(..self.cursor_position).collect::<String>();
                self.input_handler.kill_ring.push(killed);
                self.cursor_position = 0;
            }
            InputAction::KillWordBefore => {
                // Find the start of the current word
                let before = &self.input_buffer[..self.cursor_position];
                let word_start = before
                    .char_indices()
                    .rev()
                    .skip_while(|(_, c)| c.is_whitespace())
                    .find(|(_, c)| c.is_whitespace())
                    .map(|(i, _)| i + 1)
                    .unwrap_or(0);
                let killed = self.input_buffer[word_start..self.cursor_position].to_string();
                self.input_buffer.replace_range(word_start..self.cursor_position, "");
                self.cursor_position = word_start;
                if !killed.is_empty() {
                    self.input_handler.kill_ring.push(killed);
                }
            }
            InputAction::MoveLeft => {
                if self.cursor_position > 0 {
                    self.cursor_position -= 1;
                }
            }
            InputAction::MoveRight => {
                if self.cursor_position < self.input_buffer.len() {
                    self.cursor_position += 1;
                }
            }
            InputAction::MovePrevWord => {
                let before = &self.input_buffer[..self.cursor_position];
                let word_start = before
                    .char_indices()
                    .rev()
                    .skip_while(|(_, c)| c.is_whitespace())
                    .find(|(_, c)| c.is_whitespace())
                    .map(|(i, _)| i + 1)
                    .unwrap_or(0);
                self.cursor_position = word_start;
            }
            InputAction::MoveNextWord => {
                let after = &self.input_buffer[self.cursor_position..];
                let word_end = after
                    .char_indices()
                    .skip_while(|(_, c)| c.is_whitespace())
                    .find(|(_, c)| c.is_whitespace())
                    .map(|(i, _)| self.cursor_position + i)
                    .unwrap_or(self.input_buffer.len());
                self.cursor_position = word_end;
            }
            InputAction::MoveStartOfLine => {
                self.cursor_position = 0;
            }
            InputAction::MoveEndOfLine => {
                self.cursor_position = self.input_buffer.len();
            }
            InputAction::MoveUp | InputAction::MoveDown => {
                // History navigation — placeholder
            }
            InputAction::Yank => {
                if let Some(text) = self.input_handler.kill_ring.current() {
                    let text = text.to_string();
                    self.input_buffer
                        .insert_str(self.cursor_position, &text);
                    self.cursor_position += text.len();
                }
            }
            InputAction::YankPop => {
                self.input_handler.kill_ring.rotate_back();
                if let Some(text) = self.input_handler.kill_ring.current() {
                    let text = text.to_string();
                    self.input_buffer
                        .insert_str(self.cursor_position, &text);
                    self.cursor_position += text.len();
                }
            }
            InputAction::Submit => {
                if !self.input_buffer.trim().is_empty() {
                    let input = self.input_buffer.clone();
                    self.messages
                        .push(format!("› {}", input.replace('\n', "\n  ")));
                    self.messages.push("  Processing...".to_string());
                    self.input_buffer.clear();
                    self.cursor_position = 0;

                    // Update state
                    {
                        let mut state = write_state(&self.state);
                        state.is_querying = true;
                    }

                    // Simulate response (placeholder)
                    self.messages
                        .push("  (Query engine not yet connected)".to_string());
                    {
                        let mut state = write_state(&self.state);
                        state.is_querying = false;
                    }
                }
            }
            InputAction::Cancel => {
                self.input_buffer.clear();
                self.cursor_position = 0;
            }
            InputAction::Exit => {
                self.is_running = false;
            }
            InputAction::Redraw => {
                let _ = self.terminal.force_redraw();
            }
            InputAction::ToggleTranscript => {
                let mut state = write_state(&self.state);
                state.transcript_mode = !state.transcript_mode;
            }
            InputAction::ToggleShowAll => {
                let mut state = write_state(&self.state);
                state.show_all_in_transcript = !state.show_all_in_transcript;
            }
            InputAction::HistorySearch => {
                // Placeholder
            }
            InputAction::AcceptAutocomplete
            | InputAction::DismissAutocomplete
            | InputAction::PreviousAutocomplete
            | InputAction::NextAutocomplete => {
                // Placeholder
            }
            InputAction::Confirm(yes) => {
                // Placeholder for y/n dialogs
            }
            InputAction::Suspend => {
                // Placeholder
            }
            InputAction::Unknown => {}
        }
    }

    /// Render the TUI frame (static — no self borrow).
    fn render_frame_static(
        frame: &mut Frame,
        messages: &[String],
        input_buffer: &str,
        cursor_position: usize,
        is_querying: bool,
    ) {
        // Main layout: messages area + input area
        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Min(3), // Messages area
                Constraint::Length(1), // Spinner/status
                Constraint::Length(4), // Input area (border + input + footer)
            ])
            .split(frame.area());

        // Render messages
        Self::render_messages_static(frame, main_layout[0], messages);

        // Render spinner/status
        Self::render_spinner_static(frame, main_layout[1], is_querying);

        // Render input
        Self::render_input_static(frame, main_layout[2], input_buffer, cursor_position);
    }

    /// Render the messages area.
    fn render_messages_static(frame: &mut Frame, area: ratatui::layout::Rect, messages: &[String]) {
        let lines: Vec<Line> = messages
            .iter()
            .map(|msg| {
                if msg.starts_with("› ") {
                    // User message
                    Line::from(vec![
                        Span::styled("› ", Style::default().fg(ratatui::style::Color::Blue)),
                        Span::raw(&msg[2..]),
                    ])
                } else if msg.starts_with("  ") {
                    // Assistant/system message
                    Line::from(vec![Span::styled(
                        msg.clone(),
                        Style::default().fg(ratatui::style::Color::DarkGray),
                    )])
                } else {
                    Line::from(msg.clone())
                }
            })
            .collect();

        let text = Text::from(lines);
        let paragraph = Paragraph::new(text).wrap(Wrap { trim: false });
        frame.render_widget(paragraph, area);
    }

    /// Render the spinner/status line.
    fn render_spinner_static(frame: &mut Frame, area: ratatui::layout::Rect, is_querying: bool) {
        let status = if is_querying {
            "⠋ Requesting…"
        } else {
            "∗ Idle"
        };

        let paragraph = Paragraph::new(status).style(
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM),
        );
        frame.render_widget(paragraph, area);
    }

    /// Render the input area.
    fn render_input_static(frame: &mut Frame, area: ratatui::layout::Rect, input_buffer: &str, cursor_position: usize) {
        // Input layout: border box + input line + footer
        let input_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1), // Border line
                Constraint::Length(1), // Input line
                Constraint::Length(1), // Footer
            ])
            .split(area);

        // Border line (round style, bottom only)
        let border_line = "─".repeat(area.width as usize);
        let border = Paragraph::new(border_line).style(
            Style::default().fg(ratatui::style::Color::DarkGray),
        );
        frame.render_widget(border, input_layout[0]);

        // Input line with cursor
        let cursor_char = " ";
        let cursor_style = Style::default()
            .bg(ratatui::style::Color::White)
            .fg(ratatui::style::Color::Black);

        let cp = cursor_position.min(input_buffer.len());
        let before = &input_buffer[..cp];
        let after = &input_buffer[cp..];

        let input_line = Line::from(vec![
            Span::styled("› ", Style::default().fg(ratatui::style::Color::Blue)),
            Span::raw(before),
            Span::styled(cursor_char, cursor_style),
            Span::raw(after),
        ]);

        let input_paragraph = Paragraph::new(input_line);
        frame.render_widget(input_paragraph, input_layout[1]);

        // Footer
        let footer = "default · ctrl+c exit · ctrl+o transcript";
        let footer_paragraph = Paragraph::new(footer).style(
            Style::default()
                .fg(ratatui::style::Color::DarkGray)
                .add_modifier(Modifier::DIM),
        );
        frame.render_widget(footer_paragraph, input_layout[2]);
    }
}

/// Run the TUI application.
pub fn run_tui() -> io::Result<()> {
    let mut app = TuiApp::new()?;
    app.run()
}
