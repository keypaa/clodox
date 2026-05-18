use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::theme::Theme;
use crate::state::AppState;
use crate::components::messages::row::{RenderMessage, render_messages};
use crate::components::spinner::with_verb::{SpinnerWithVerb, IdleStatus};
use crate::components::prompt_input::text_input::TextInputWidget;
use crate::components::prompt_input::footer::{PromptFooter, PromptMode};
use crate::components::prompt_input::autocomplete::{AutocompleteWidget, AutocompleteState};
use crate::screens::logo_header::{LogoHeader, LogoHeaderWidget};
use crate::virtual_scroll::{VirtualMessageList, NewMessagesPill};

pub struct FullscreenScreen {
    theme: Theme,
    virtual_list: VirtualMessageList,
    is_querying: bool,
    reduced_motion: bool,
    has_rendered: bool,
}

impl FullscreenScreen {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            virtual_list: VirtualMessageList::new(),
            is_querying: false,
            reduced_motion: false,
            has_rendered: false,
        }
    }

    pub fn update(&mut self, state: &AppState) {
        let messages: Vec<RenderMessage> = state.messages.iter()
            .map(|msg| message_to_render_message(msg))
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
            let spinner = SpinnerWithVerb::new(self.reduced_motion)
                .with_override_message("Requesting".to_string());
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
                let footer = PromptFooter::new()
                    .with_mode(PromptMode::Default)
                    .with_dimensions(false, is_narrow);
                frame.render_widget(footer, footer_area);
            }
        }
    }
}

fn message_to_render_message(msg: &cc_core::messages::Message) -> RenderMessage {
    use cc_core::messages::Message;

    match msg {
        Message::User(user_msg) => {
            let text = user_msg.content.iter()
                .filter_map(|block| match block {
                    cc_core::messages::ContentBlockParam::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            RenderMessage::UserText { text }
        }
        Message::Assistant(assistant_msg) => {
            let text = assistant_msg.content.iter()
                .filter_map(|block| match block {
                    cc_core::messages::ContentBlockParam::Text { text } => Some(text.clone()),
                    _ => None,
                })
                .collect::<Vec<_>>()
                .join("\n");

            if !text.is_empty() {
                RenderMessage::AssistantText { text }
            } else {
                let tool_use = assistant_msg.content.iter()
                    .find_map(|block| match block {
                        cc_core::messages::ContentBlockParam::ToolUse { name, input, .. } => {
                            Some((name.clone(), input.clone()))
                        }
                        _ => None,
                    });

                match tool_use {
                    Some((name, _input)) => {
                        RenderMessage::AssistantToolUse {
                            tool_name: name,
                            details: None,
                            status: Some("Running…".to_string()),
                            is_resolved: false,
                            is_error: false,
                        }
                    }
                    None => RenderMessage::AssistantText { text: String::new() },
                }
            }
        }
        Message::System(system_msg) => {
            let text = match system_msg {
                cc_core::messages::SystemMessage::Informational(msg) => msg.text.clone(),
                cc_core::messages::SystemMessage::ApiError(msg) => msg.error.clone(),
                _ => String::new(),
            };

            RenderMessage::SystemError { error: text }
        }
        _ => RenderMessage::AssistantText { text: String::new() },
    }
}
