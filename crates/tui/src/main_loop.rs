use std::env;
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
use crate::screens::{ReplScreen, FullscreenScreen};
use crate::components::prompt_input::autocomplete::AutocompleteState;

enum ScreenMode {
    Repl(ReplScreen),
    Fullscreen(FullscreenScreen),
}

impl ScreenMode {
    fn from_env(theme: Theme) -> Self {
        if env::var("CLAUDE_CODE_FULLSCREEN")
            .map(|v| v == "1" || v == "true")
            .unwrap_or(false)
        {
            ScreenMode::Fullscreen(FullscreenScreen::new(theme))
        } else {
            ScreenMode::Repl(ReplScreen::new(theme))
        }
    }

    fn update(&mut self, state: &crate::state::AppState) {
        match self {
            ScreenMode::Repl(screen) => screen.update(state),
            ScreenMode::Fullscreen(screen) => screen.update(state),
        }
    }

    fn set_reduced_motion(&mut self, enabled: bool) {
        match self {
            ScreenMode::Repl(screen) => screen.set_reduced_motion(enabled),
            ScreenMode::Fullscreen(screen) => screen.set_reduced_motion(enabled),
        }
    }
}

pub struct TuiApp {
    pub terminal: TerminalManager,
    pub state: SharedState,
    pub input_handler: InputHandler,
    pub theme: Theme,
    pub input_buffer: String,
    pub cursor_position: usize,
    pub is_running: bool,
    pub screen: ScreenMode,
    pub autocomplete: AutocompleteState,
}

impl TuiApp {
    pub fn new() -> io::Result<Self> {
        let terminal = TerminalManager::new()?;
        let state = create_state();
        let input_handler = InputHandler::new();
        let theme = Theme::default();
        let screen = ScreenMode::from_env(theme.clone());

        Ok(Self {
            terminal,
            state,
            input_handler,
            theme,
            input_buffer: String::new(),
            cursor_position: 0,
            is_running: true,
            screen,
            autocomplete: AutocompleteState::new(),
        })
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.terminal.initialize()?;

        while self.is_running {
            let has_event = self.terminal.poll_event(Duration::from_millis(50))?;
            if has_event {
                let event = self.terminal.read_event()?;
                let action = self.input_handler.process_event(event);
                if let Some(action) = action {
                    self.handle_action(action);
                }
            }

            let input_buffer = self.input_buffer.clone();
            let cursor_position = self.cursor_position;
            let autocomplete = self.autocomplete.clone();

            self.terminal.draw(|frame| {
                let area = frame.area();

                if let ScreenMode::Repl(screen) = &self.screen {
                    screen.render(frame, area, &input_buffer, cursor_position, &autocomplete);
                } else if let ScreenMode::Fullscreen(screen) = &mut self.screen {
                    screen.render(frame, area, &input_buffer, cursor_position, &autocomplete);
                }
            })?;
        }

        self.terminal.cleanup()?;
        Ok(())
    }

    fn handle_action(&mut self, action: InputAction) {
        match action {
            InputAction::InsertChar(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
                self.autocomplete.update(&self.input_buffer, &crate::components::prompt_input::builtin_commands());
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
            InputAction::MoveUp => {
                if self.autocomplete.is_active() {
                    self.autocomplete.select_prev();
                } else {
                    if let ScreenMode::Fullscreen(screen) = &mut self.screen {
                        screen.scroll_up(3);
                    }
                }
            }
            InputAction::MoveDown => {
                if self.autocomplete.is_active() {
                    self.autocomplete.select_next();
                } else {
                    if let ScreenMode::Fullscreen(screen) = &mut self.screen {
                        screen.scroll_down(3);
                    }
                }
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
                    self.input_buffer.clear();
                    self.cursor_position = 0;
                    self.autocomplete.dismiss();

                    {
                        let mut state = write_state(&self.state);
                        state.is_querying = true;
                    }

                    if let ScreenMode::Fullscreen(screen) = &mut self.screen {
                        screen.scroll_to_bottom();
                    }
                }
            }
            InputAction::Cancel => {
                self.input_buffer.clear();
                self.cursor_position = 0;
                self.autocomplete.dismiss();
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
            InputAction::HistorySearch => {}
            InputAction::AcceptAutocomplete => {
                if self.autocomplete.is_active() {
                    if let Some(cmd) = self.autocomplete.accept() {
                        self.input_buffer = cmd;
                        self.cursor_position = self.input_buffer.len();
                    }
                }
            }
            InputAction::DismissAutocomplete => {
                self.autocomplete.dismiss();
            }
            InputAction::PreviousAutocomplete => {
                self.autocomplete.select_prev();
            }
            InputAction::NextAutocomplete => {
                self.autocomplete.select_next();
            }
            InputAction::Confirm(yes) => {}
            InputAction::Suspend => {}
            InputAction::Unknown => {}
        }
    }
}

pub fn run_tui() -> io::Result<()> {
    let mut app = TuiApp::new()?;
    app.run()
}
