use std::env;
use std::io;
use std::sync::Arc;
use std::time::Duration;

use cc_core::messages::{ContentBlockParam, Message, UserMessage};
use cc_core::state::QueryState;
use cc_core::permissions::{PermissionMode, RiskLevel, ToolPermissionContext};
use cc_query::engine::{QueryConfig, QueryEngine, QueryEvent, TokenBudget};
use cc_query::api_client::ApiConfig;
use cc_query::retry::RetryOptions;
use cc_query::system_prompt::{assemble_system_prompt, SystemPromptConfig};
use cc_tools::registry::ToolRegistry;
use crossterm::event::Event;
use ratatui::{
    layout::{Constraint, Direction, Layout},
    style::{Modifier, Style},
    text::{Line, Span, Text},
    widgets::{Block, Borders, Paragraph, Wrap},
    Frame,
};
use tokio::sync::mpsc;

use crate::input::{InputAction, InputHandler};
use crate::state::{create_state, write_state, SharedState};
use crate::terminal::TerminalManager;
use crate::theme::Theme;
use crate::screens::{ReplScreen, FullscreenScreen};
use crate::components::prompt_input::autocomplete::AutocompleteState;
use crate::query_events::StreamingAccumulator;
use crate::components::permissions::dialog::{PermissionAction, PermissionDialog, PermissionDialogWidget};

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
    pub command_registry: cc_commands::CommandRegistry,
    pub command_context: cc_commands::CommandContext,
    pub pending_command_result: Option<String>,
    pub tool_registry: ToolRegistry,
    pub tokio_runtime: tokio::runtime::Runtime,
    pub query_event_rx: Option<mpsc::Receiver<Result<QueryEvent, cc_query::errors::QueryError>>>,
    pub query_task: Option<tokio::task::JoinHandle<()>>,
    pub abort_tx: Option<tokio::sync::watch::Sender<bool>>,
    pub query_accumulator: StreamingAccumulator,
    pub permission_mode: PermissionMode,
    pub permission_context: ToolPermissionContext,
}

impl TuiApp {
    pub fn new() -> io::Result<Self> {
        let terminal = TerminalManager::new()?;
        let state = create_state();
        let input_handler = InputHandler::new();
        let theme = Theme::default();
        let screen = ScreenMode::from_env(theme.clone());

        let mut command_registry = cc_commands::CommandRegistry::new();
        cc_commands::register_all_commands(&mut command_registry);

        let command_context = cc_commands::CommandContext::new(state.clone());

        let tool_registry = ToolRegistry::default_registry();

        let tokio_runtime = tokio::runtime::Builder::new_multi_thread()
            .worker_threads(2)
            .thread_name("query-worker")
            .enable_all()
            .build()
            .expect("Failed to create tokio runtime");

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
            command_registry,
            command_context,
            pending_command_result: None,
            tool_registry,
            tokio_runtime,
            query_event_rx: None,
            query_task: None,
            abort_tx: None,
            query_accumulator: StreamingAccumulator::new(),
            permission_mode: PermissionMode::Default,
            permission_context: ToolPermissionContext::default(),
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

            self.poll_query_events();

            let state_snapshot = {
                let s = self.state.read().expect("state lock poisoned");
                s.clone()
            };

            self.input_handler.is_query_active = state_snapshot.query_state == QueryState::Sending
                || state_snapshot.query_state == QueryState::Streaming
                || state_snapshot.query_state == QueryState::ToolPending
                || state_snapshot.query_state == QueryState::ToolRunning
                || state_snapshot.query_state == QueryState::Compacting;

            if state_snapshot.query_state == cc_core::state::QueryState::Streaming
                || state_snapshot.query_state == cc_core::state::QueryState::ToolRunning {
            }

            let input_buffer = self.input_buffer.clone();
            let cursor_position = self.cursor_position;
            let autocomplete = self.autocomplete.clone();

            self.terminal.draw(|frame| {
                let area = frame.area();

                if let ScreenMode::Repl(screen) = &self.screen {
                    screen.render(frame, area, &input_buffer, cursor_position, &autocomplete);
                } else if let ScreenMode::Fullscreen(screen) = &mut self.screen {
                    screen.render(frame, area, &input_buffer, cursor_position, &autocomplete, &state_snapshot);
                }
            })?;
        }

        self.terminal.cleanup()?;
        Ok(())
    }

    fn handle_action(&mut self, action: InputAction) {
        if self.is_permission_dialog_active() {
            self.handle_permission_action(action);
            return;
        }

        match action {
            InputAction::InsertChar(c) => {
                self.input_buffer.insert(self.cursor_position, c);
                self.cursor_position += 1;
                if self.input_buffer.starts_with('/') {
                    let suggestions = cc_commands::get_suggestions(&self.command_registry, &self.input_buffer, 10);
                    self.autocomplete.update_with_suggestions(&self.input_buffer, suggestions);
                } else {
                    self.autocomplete.update(&self.input_buffer, &crate::components::prompt_input::builtin_commands());
                }
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

                    if cc_commands::is_slash_command(&input) {
                        let registry = &self.command_registry;
                        let ctx = &self.command_context;
                        let rt = tokio::runtime::Runtime::new().unwrap();
                        let result = rt.block_on(cc_commands::execute_command(registry, ctx, &input));
                        match result {
                            cc_commands::CommandResult::Text { message, .. } => {
                                self.pending_command_result = Some(message);
                            }
                            cc_commands::CommandResult::Error { message } => {
                                self.pending_command_result = Some(format!("Error: {message}"));
                            }
                            cc_commands::CommandResult::Skip => {}
                            cc_commands::CommandResult::Compact { display_text, .. } => {
                                self.pending_command_result = display_text.or_else(|| Some("Context compacted.".to_string()));
                            }
                            cc_commands::CommandResult::Prompt { .. } => {
                                self.pending_command_result = Some("Prompt command executed.".to_string());
                            }
                            cc_commands::CommandResult::Navigate { screen } => {
                                match screen.as_str() {
                                    "login" => {
                                        self.screen = ScreenMode::Fullscreen(FullscreenScreen::new_login(self.theme.clone()));
                                    }
                                    "resume" => {
                                        self.screen = ScreenMode::Fullscreen(FullscreenScreen::new_resume(self.theme.clone()));
                                    }
                                    _ => {
                                        self.pending_command_result = Some(format!("Unknown screen: {screen}"));
                                    }
                                }
                            }
                        }
                    } else if !self.is_query_active() {
                        self.spawn_query(input.trim().to_string());

                        if let ScreenMode::Fullscreen(screen) = &mut self.screen {
                            screen.scroll_to_bottom();
                        }
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
            InputAction::AbortQuery => {
                if let Some(tx) = &self.abort_tx {
                    let _ = tx.send(true);
                    let mut state = write_state(&self.state);
                    state.query_state = QueryState::Cancelling;
                }
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
            InputAction::Confirm(yes) => {
                let mut state = write_state(&self.state);
                if state.pending_permission_dialog.is_some() {
                    let action = if yes {
                        PermissionAction::AllowOnce
                    } else {
                        PermissionAction::Deny
                    };
                    state.pending_permission_dialog = None;
                    state.query_state = QueryState::Streaming;
                }
            }
            InputAction::Suspend => {}
            InputAction::Unknown => {}
        }
    }

    fn is_query_active(&self) -> bool {
        self.query_task.is_some()
    }

    fn is_permission_dialog_active(&self) -> bool {
        let state = self.state.read().expect("state lock poisoned");
        state.pending_permission_dialog.is_some()
    }

    fn handle_permission_action(&mut self, action: InputAction) {
        match action {
            InputAction::Confirm(true) => {
                let mut state = write_state(&self.state);
                state.pending_permission_dialog = None;
                if state.query_state == QueryState::ToolRunning {
                    state.query_state = QueryState::Streaming;
                }
            }
            InputAction::Confirm(false) => {
                let mut state = write_state(&self.state);
                state.pending_permission_dialog = None;
                state.messages.push(Message::User(UserMessage {
                    id: uuid::Uuid::new_v4(),
                    content: vec![ContentBlockParam::Text {
                        text: "Tool call denied by user.".to_string(),
                    }],
                    timestamp: chrono::Utc::now(),
                    is_meta: None,
                    origin_query_source: None,
                    effort: None,
                }));
            }
            InputAction::Cancel | InputAction::Exit => {
                let mut state = write_state(&self.state);
                state.pending_permission_dialog = None;
            }
            _ => {}
        }
    }

    fn spawn_query(&mut self, text: String) {
        let user_message = UserMessage {
            id: uuid::Uuid::new_v4(),
            content: vec![ContentBlockParam::Text { text }],
            timestamp: chrono::Utc::now(),
            is_meta: None,
            origin_query_source: None,
            effort: None,
        };

        {
            let mut state = write_state(&self.state);
            state.messages.push(cc_core::messages::Message::User(user_message.clone()));
            state.query_state = QueryState::Sending;
            state.is_querying = true;
            state.query_error = None;
        }

        let (abort_tx, abort_rx) = tokio::sync::watch::channel(false);
        self.abort_tx = Some(abort_tx);

        let state_clone = self.state.clone();
        let tools: cc_core::tools::Tools = Arc::new(
            self.tool_registry.all().into_iter().map(|t| t as Arc<dyn cc_core::tools::Tool>).collect(),
        );
        let model = {
            let state = state_clone.read().expect("state lock poisoned");
            state.main_loop_model.name.clone()
        };
        let api_key = env::var("ANTHROPIC_API_KEY").unwrap_or_default();
        let system_prompt = assemble_system_prompt(
            &SystemPromptConfig::default(),
            &tools,
        );

        let config = QueryConfig {
            model,
            max_tokens: 4096,
            system_prompt,
            tools,
            permission_context: self.permission_context.clone(),
            temperature: None,
            thinking_enabled: false,
            thinking_budget: None,
            token_budget: TokenBudget::new(None),
            api_config: ApiConfig {
                api_key,
                base_url: "https://api.anthropic.com".to_string(),
                timeout: Duration::from_secs(600),
                anthropic_version: "2023-06-01".to_string(),
                betas: vec!["prompt-caching-2024-07-31".to_string()],
            },
            retry_options: RetryOptions {
                max_retries: 3,
                model: "claude-sonnet-4-20250514".to_string(),
                fallback_model: None,
                initial_consecutive_529: 0,
            },
            verbose: false,
            debug: false,
        };

        let (event_tx, event_rx) = mpsc::channel(64);
        self.query_event_rx = Some(event_rx);

        let handle = self.tokio_runtime.handle().clone();
        let task = handle.spawn(async move {
            let mut query_engine = match QueryEngine::new(config, abort_rx) {
                Ok(e) => e,
                Err(e) => {
                    let _ = event_tx.send(Err(e)).await;
                    return;
                }
            };

            use futures::StreamExt;
            let stream = query_engine.submit_message(user_message).await;
            let mut pinned = std::pin::pin!(stream);
            while let Some(event) = pinned.next().await {
                if event_tx.send(event).await.is_err() {
                    break;
                }
            }
        });

        self.query_task = Some(task);
    }

    fn poll_query_events(&mut self) {
        let events = if let Some(rx) = &mut self.query_event_rx {
            let mut events = Vec::new();
            while let Ok(event_result) = rx.try_recv() {
                events.push(event_result);
            }
            events
        } else {
            Vec::new()
        };

        for event_result in events {
            crate::query_events::handle_query_event(
                event_result,
                &self.state,
                &mut self.query_accumulator,
                self.permission_mode,
                &self.permission_context,
            );
        }

        if let Some(task) = &self.query_task {
            if task.is_finished() {
                self.query_task = None;
                self.abort_tx = None;
                self.query_event_rx = None;

                let mut state = write_state(&self.state);
                if state.query_state != QueryState::Error {
                    state.query_state = QueryState::Idle;
                    state.is_querying = false;
                }
            }
        }
    }

}

pub fn run_tui() -> io::Result<()> {
    let mut app = TuiApp::new()?;
    app.run()
}
