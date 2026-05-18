use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Paragraph, Wrap};

use crate::theme::Theme;
use crate::state::AppState;
use crate::components::messages::row::{RenderMessage, render_messages};
use crate::components::messages::converter::core_message_to_render_message;
use crate::components::spinner::with_verb::{SpinnerWithVerb, IdleStatus};
use crate::components::prompt_input::text_input::TextInputWidget;
use crate::components::prompt_input::footer::{PromptFooter, PromptMode};
use crate::components::prompt_input::autocomplete::{AutocompleteWidget, AutocompleteState};
use crate::screens::logo_header::{LogoHeader, LogoHeaderWidget};
use crate::screens::welcome::{WelcomeScreen, WelcomeWidget};

const MAX_MESSAGES_REPL: usize = 200;

pub struct ReplScreen {
    theme: Theme,
    messages: Vec<RenderMessage>,
    is_querying: bool,
    reduced_motion: bool,
    welcome_screen: WelcomeScreen,
}

impl ReplScreen {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            messages: Vec::new(),
            is_querying: false,
            reduced_motion: false,
            welcome_screen: WelcomeScreen::new(),
        }
    }

    pub fn update(&mut self, state: &AppState) {
        let capped = if state.messages.len() > MAX_MESSAGES_REPL {
            state.messages[state.messages.len() - MAX_MESSAGES_REPL..].to_vec()
        } else {
            state.messages.clone()
        };

        self.messages = capped
            .into_iter()
            .map(|msg| core_message_to_render_message(&msg))
            .collect();

        self.is_querying = state.is_querying;
    }

    pub fn set_reduced_motion(&mut self, enabled: bool) {
        self.reduced_motion = enabled;
    }

    pub fn render(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        input_text: &str,
        cursor_pos: usize,
        autocomplete: &AutocompleteState,
    ) {
        let columns = area.width;
        let is_narrow = columns < 80;

        let has_messages = !self.messages.is_empty();
        let logo_height = if has_messages { 12 } else { 0 };

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(logo_height),
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(4),
            ])
            .split(area);

        let mut layout_idx = 0;

        if has_messages {
            let logo_header = LogoHeader::new();
            let logo = LogoHeaderWidget::new(
                &logo_header,
                &self.theme,
                columns,
            );
            frame.render_widget(logo, main_layout[layout_idx]);
            layout_idx += 1;
        } else {
            // Show welcome dialog when no messages
            let welcome_widget = WelcomeWidget::new(&self.welcome_screen, &self.theme);
            frame.render_widget(welcome_widget, area);
        }

        if !self.messages.is_empty() {
            render_messages(
                frame,
                main_layout[layout_idx],
                &self.messages,
                &self.theme,
                None,
            );
            layout_idx += 1;
        }

        if self.is_querying {
            let spinner = SpinnerWithVerb::new(self.reduced_motion)
                .with_override_message("Requesting".to_string());
            frame.render_widget(spinner, main_layout[layout_idx]);
            layout_idx += 1;
        } else {
            let idle = IdleStatus::new(self.reduced_motion);
            frame.render_widget(idle, main_layout[layout_idx]);
            layout_idx += 1;
        }

        self.render_prompt_input(
            frame,
            main_layout[layout_idx],
            input_text,
            cursor_pos,
            autocomplete,
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
        is_narrow: bool,
    ) {
        let input_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(1),
                Constraint::Length(1),
                Constraint::Length(1),
            ])
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

        let cp = cursor_pos.min(input_text.chars().count());
        let byte_idx = input_text
            .char_indices()
            .nth(cp)
            .map(|(i, _)| i)
            .unwrap_or(input_text.len());
        let before = &input_text[..byte_idx];
        let after = &input_text[byte_idx..];

        let input_line = Line::from(vec![
            Span::styled("› ", Style::default().fg(self.theme.colors.suggestion)),
            Span::raw(before),
            Span::styled(cursor_char, cursor_style),
            Span::raw(after),
        ]);

        let input_paragraph = Paragraph::new(input_line);
        frame.render_widget(input_paragraph, input_layout[1]);

        if autocomplete.is_active() {
            let ac_widget = AutocompleteWidget::new(autocomplete, &self.theme);
            frame.render_widget(ac_widget, input_layout[2]);
        } else {
            let footer = PromptFooter::new()
                .with_mode(PromptMode::Default)
                .with_dimensions(false, is_narrow);
            frame.render_widget(footer, input_layout[2]);
        }
    }
}

