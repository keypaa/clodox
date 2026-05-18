use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Paragraph;

use crate::theme::Theme;
use crate::state::AppState;
use crate::components::messages::row::{RenderMessage, render_messages};
use crate::components::spinner::with_verb::{SpinnerWithVerb, IdleStatus};
use crate::components::prompt_input::footer::{PromptFooter, PromptMode};
use crate::components::prompt_input::autocomplete::{AutocompleteWidget, AutocompleteState};
use crate::components::permissions::dialog::{PermissionDialog, PermissionDialogWidget};
use crate::screens::logo_header::{LogoHeader, LogoHeaderWidget};
use crate::screens::login::{LoginScreen, LoginScreenWidget};
use crate::screens::resume::{ResumePicker, ResumePickerWidget};
use crate::virtual_scroll::{VirtualMessageList, NewMessagesPill};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FullscreenMode {
    Chat,
    Login,
    Resume,
}

pub struct FullscreenScreen {
    theme: Theme,
    pub mode: FullscreenMode,
    virtual_list: VirtualMessageList,
    is_querying: bool,
    reduced_motion: bool,
    has_rendered: bool,
    login_screen: LoginScreen,
    resume_picker: ResumePicker,
}

impl FullscreenScreen {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            mode: FullscreenMode::Chat,
            virtual_list: VirtualMessageList::new(),
            is_querying: false,
            reduced_motion: false,
            has_rendered: false,
            login_screen: LoginScreen::new(),
            resume_picker: ResumePicker::new(ResumePicker::load_sessions()),
        }
    }

    pub fn new_login(theme: Theme) -> Self {
        Self {
            theme,
            mode: FullscreenMode::Login,
            virtual_list: VirtualMessageList::new(),
            is_querying: false,
            reduced_motion: false,
            has_rendered: false,
            login_screen: LoginScreen::new(),
            resume_picker: ResumePicker::new(ResumePicker::load_sessions()),
        }
    }

    pub fn new_resume(theme: Theme) -> Self {
        Self {
            theme,
            mode: FullscreenMode::Resume,
            virtual_list: VirtualMessageList::new(),
            is_querying: false,
            reduced_motion: false,
            has_rendered: false,
            login_screen: LoginScreen::new(),
            resume_picker: ResumePicker::new(ResumePicker::load_sessions()),
        }
    }

    pub fn update(&mut self, state: &AppState) {
        let messages: Vec<RenderMessage> = state.messages.iter()
            .map(|msg| core_message_to_render_message(msg))
            .collect();

        self.virtual_list.update_messages(messages);
        self.is_querying = state.is_querying;

        if state.transcript_mode {
            self.virtual_list.set_transcript_mode(true);
        }
    }

    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.reduced_motion = enabled;
    }

    pub fn handle_resize(&mut self, width: u16, height: u16) {
        self.virtual_list.resize(width, height);
    }

    pub fn scroll_up(&mut self, lines: u16) {
        self.virtual_list.scroll_up(lines);
    }

    pub fn scroll_down(&mut self, lines: u16) {
        self.virtual_list.scroll_down(lines);
    }

    pub fn scroll_to_bottom(&mut self) {
        self.virtual_list.scroll_to_bottom();
    }

    pub fn is_at_bottom(&self) -> bool {
        self.virtual_list.is_at_bottom()
    }

    pub fn render(
        &mut self,
        frame: &mut ratatui::Frame,
        area: Rect,
        input_text: &str,
        cursor_pos: usize,
        autocomplete: &AutocompleteState,
        state: &AppState,
    ) {
        match self.mode {
            FullscreenMode::Login => {
                let widget = LoginScreenWidget::new(&self.login_screen, &self.theme);
                frame.render_widget(widget, area);
            }
            FullscreenMode::Resume => {
                let widget = ResumePickerWidget::new(&self.resume_picker, &self.theme);
                frame.render_widget(widget, area);
            }
            FullscreenMode::Chat => {
                self.render_chat(frame, area, input_text, cursor_pos, autocomplete, state);
                if let Some(ref dialog_state) = state.pending_permission_dialog {
                    self.render_permission_dialog(frame, area, dialog_state);
                }
            }
        }
    }

    fn render_chat(
        &mut self,
        frame: &mut ratatui::Frame,
        area: Rect,
        input_text: &str,
        cursor_pos: usize,
        autocomplete: &AutocompleteState,
        state: &AppState,
    ) {
        let columns = area.width;
        let rows = area.height;
        let is_short = rows < 24;
        let is_narrow = columns < 80;

        if !self.has_rendered {
            self.handle_resize(columns, rows);
            self.has_rendered = true;
        }

        let logo_height = if is_short { 0 } else { 12 };
        let spinner_height = 1;
        let prompt_height = if is_short { 3 } else { 4 };

        let constraints = if is_short {
            vec![
                Constraint::Min(3),
                Constraint::Length(spinner_height),
                Constraint::Length(prompt_height),
            ]
        } else {
            vec![
                Constraint::Length(logo_height),
                Constraint::Min(3),
                Constraint::Length(spinner_height),
                Constraint::Length(prompt_height),
            ]
        };

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let mut layout_idx = 0;

        if !is_short {
            let logo_header = LogoHeader::new();
            let logo = LogoHeaderWidget::new(
                &logo_header,
                &self.theme,
                columns,
            );
            frame.render_widget(logo, main_layout[layout_idx]);
            layout_idx += 1;
        }

        let messages_area = main_layout[layout_idx];
        layout_idx += 1;

        if !self.virtual_list.message_count() == 0 {
            let placeholder = Paragraph::new("Start a conversation...").style(
                Style::default()
                    .fg(self.theme.colors.inactive)
                    .add_modifier(Modifier::DIM),
            );
            frame.render_widget(placeholder, messages_area);
        } else {
            render_messages(
                frame,
                messages_area,
                &self.virtual_list.messages_for_render(),
                &self.theme,
                None,
            );

            if !self.virtual_list.is_at_bottom() {
                let pill_area = Rect {
                    x: messages_area.x,
                    y: messages_area.bottom().saturating_sub(3),
                    width: messages_area.width,
                    height: 2,
                };
                let pill = NewMessagesPill::new(self.virtual_list.new_messages_count());
                frame.render_widget(pill, pill_area);
            }
        }

        let spinner_area = main_layout[layout_idx];
        layout_idx += 1;

        if self.is_querying {
            let (verb, subject) = match state.query_state {
                cc_core::state::QueryState::Sending => ("Requesting", None),
                cc_core::state::QueryState::Streaming => {
                    if state.streaming_thinking.is_some() {
                        ("Thinking", None)
                    } else {
                        ("Processing", None)
                    }
                }
                cc_core::state::QueryState::ToolPending => ("Waiting", Some("permission")),
                cc_core::state::QueryState::ToolRunning => {
                    if let Some(tc) = state.pending_tool_calls.first() {
                        ("Running", Some(tc.display_text.as_str()))
                    } else {
                        ("Running", None)
                    }
                }
                cc_core::state::QueryState::Compacting => ("Compacting", Some("context")),
                cc_core::state::QueryState::Cancelling => ("Cancelling", None),
                cc_core::state::QueryState::Error => ("Error", None),
                cc_core::state::QueryState::Idle => ("Working", None),
            };

            let mut spinner = SpinnerWithVerb::new(self.reduced_motion)
                .with_override_message(verb.to_string());

            if let Some(subj) = subject {
                spinner = spinner.with_subject(subj.to_string());
            }

            frame.render_widget(spinner, spinner_area);
        } else {
            let idle = IdleStatus::new(self.reduced_motion);
            frame.render_widget(idle, spinner_area);
        }

        let prompt_area = main_layout[layout_idx];

        self.render_prompt_input(
            frame,
            prompt_area,
            input_text,
            cursor_pos,
            autocomplete,
            is_short,
            is_narrow,
            state,
        );
    }

    fn render_prompt_input(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        input_text: &str,
        cursor_pos: usize,
        autocomplete: &AutocompleteState,
        is_short: bool,
        is_narrow: bool,
        state: &AppState,
    ) {
        let constraints = if is_short {
            vec![
                Constraint::Length(1),
                Constraint::Length(1),
            ]
        } else {
            vec![
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ]
        };

        let input_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints(constraints)
            .split(area);

        let border_line = "─".repeat(area.width as usize);
        let border = Paragraph::new(border_line).style(
            Style::default().fg(self.theme.colors.prompt_border),
        );
        frame.render_widget(border, input_layout[0]);

        let cursor_char = " ";
        let cursor_style = Style::default()
            .bg(self.theme.colors.text)
            .fg(self.theme.colors.user_message_background);

        let cp = cursor_pos.min(input_text.len());
        let before = &input_text[..cp];
        let after = &input_text[cp..];

        let input_line = Line::from(vec![
            Span::styled("› ", Style::default().fg(self.theme.colors.suggestion)),
            Span::raw(before),
            Span::styled(cursor_char, cursor_style),
            Span::raw(after),
        ]);

        let input_paragraph = Paragraph::new(input_line);
        frame.render_widget(input_paragraph, input_layout[1]);

        if !is_short {
            let footer_area = input_layout[2];

            if autocomplete.is_active() {
                let ac_widget = AutocompleteWidget::new(autocomplete, &self.theme);
                frame.render_widget(ac_widget, footer_area);
            } else {
                use crate::components::prompt_input::footer::TokenDisplay;

                let tokens = TokenDisplay {
                    input: state.token_counts.input_tokens,
                    output: state.token_counts.output_tokens,
                    cache_read: state.token_counts.cache_read_tokens,
                    cache_creation: state.token_counts.cache_creation_tokens,
                };

                let footer = PromptFooter::new()
                    .with_mode(PromptMode::Default)
                    .with_dimensions(false, is_narrow)
                    .with_tokens(tokens)
                    .with_cost(state.total_cost_usd)
                    .with_querying(state.is_querying);
                frame.render_widget(footer, footer_area);
            }
        }
    }

    fn render_permission_dialog(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        dialog_state: &cc_core::state::PermissionDialogState,
    ) {
        use cc_core::permissions::RiskLevel;

        let risk = match dialog_state.tool_name.as_str() {
            "bash" => {
                if dialog_state.tool_display.contains("rm ") || dialog_state.tool_display.contains("sudo ") {
                    RiskLevel::High
                } else if dialog_state.tool_display.contains("git ") || dialog_state.tool_display.contains("ls ") {
                    RiskLevel::Low
                } else {
                    RiskLevel::Medium
                }
            }
            "write" | "edit" => RiskLevel::Medium,
            "read" | "grep" | "glob" => RiskLevel::Low,
            _ => RiskLevel::Medium,
        };

        let dialog = PermissionDialog::new(
            &format!("Allow {}?", dialog_state.tool_name),
            &format!("This tool call requires your approval:\n\n{}", dialog_state.tool_display),
            &dialog_state.tool_display,
            risk,
        );

        let widget = PermissionDialogWidget::new(&dialog, &self.theme, area);
        frame.render_widget(widget, area);
    }
}

use crate::components::messages::converter::core_message_to_render_message;
