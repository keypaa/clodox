use ratatui::layout::{Constraint, Direction, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{Block, BorderType, Borders, Paragraph};

use crate::theme::Theme;
use crate::state::AppState;
use crate::components::messages::row::{RenderMessage, render_messages};
use crate::components::messages::converter::core_message_to_render_message;
use crate::components::spinner::with_verb::{SpinnerWithVerb, IdleStatus};
use crate::components::prompt_input::footer::{PromptFooter, PromptMode};
use crate::components::prompt_input::autocomplete::{AutocompleteWidget, AutocompleteState};

const MAX_MESSAGES_REPL: usize = 200;

pub struct ReplScreen {
    theme: Theme,
    messages: Vec<RenderMessage>,
    is_querying: bool,
    reduced_motion: bool,
}

impl ReplScreen {
    pub fn new(theme: Theme) -> Self {
        Self {
            theme,
            messages: Vec::new(),
            is_querying: false,
            reduced_motion: false,
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

        let accent_color = ratatui::style::Color::Rgb(255, 135, 0);

        let main_layout = Layout::default()
            .direction(Direction::Vertical)
            .constraints([
                Constraint::Length(10),
                Constraint::Min(3),
                Constraint::Length(1),
                Constraint::Length(4),
            ])
            .split(area);

        // Render the orange welcome block
        let welcome_block = Block::default()
            .borders(Borders::ALL)
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(accent_color))
            .title(Line::from(vec![
                Span::styled(
                    " Claude Code v0.1.0 ",
                    Style::default().fg(accent_color).add_modifier(Modifier::BOLD),
                ),
            ]));

        let welcome_inner = welcome_block.inner(main_layout[0]);
        frame.render_widget(welcome_block, main_layout[0]);

        // Render welcome content inside the block
        self.render_welcome_content(frame, welcome_inner, accent_color);

        // Render messages area
        if !self.messages.is_empty() {
            render_messages(
                frame,
                main_layout[1],
                &self.messages,
                &self.theme,
                None,
            );
        }

        // Render spinner or idle status
        if self.is_querying {
            let spinner = SpinnerWithVerb::new(self.reduced_motion)
                .with_override_message("Requesting".to_string());
            frame.render_widget(spinner, main_layout[2]);
        } else {
            let idle = IdleStatus::new(self.reduced_motion);
            frame.render_widget(idle, main_layout[2]);
        }

        // Render prompt input
        self.render_prompt_input(
            frame,
            main_layout[3],
            input_text,
            cursor_pos,
            autocomplete,
            is_narrow,
        );
    }

    fn render_welcome_content(
        &self,
        frame: &mut ratatui::Frame,
        area: Rect,
        accent_color: ratatui::style::Color,
    ) {
        let layout = Layout::default()
            .direction(Direction::Horizontal)
            .constraints([
                Constraint::Percentage(55),
                Constraint::Percentage(45),
            ])
            .split(area);

        let left_block = Block::default()
            .borders(Borders::RIGHT)
            .border_style(Style::default().fg(accent_color));

        let left_inner = left_block.inner(layout[0]);
        frame.render_widget(left_block, layout[0]);

        let dim_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);

        let welcome_style = Style::default()
            .fg(ratatui::style::Color::White)
            .add_modifier(Modifier::BOLD);

        let alien_style = Style::default().fg(accent_color);

        let left_width = left_inner.width as usize;

        let welcome = "Welcome back my Thane!";
        let welcome_padding = left_width.saturating_sub(welcome.len()) / 2;

        let alien = vec![
            "  ▐▛███▜▌  ",
            " ▝▜█████▛▘ ",
            "   ▘▘ ▝   ",
        ];

        let model_info = vec![
            "Sonnet 4.6 · Claude Pro · Organization",
            "~/GitHub/simplespace",
        ];

        let mut left_content = Vec::new();

        left_content.push(Line::from(vec![
            Span::raw(" ".repeat(welcome_padding)),
            Span::styled(welcome, welcome_style),
        ]));

        left_content.push(Line::from(vec![Span::raw("")]));

        for alien_line in &alien {
            let alien_padding = left_width.saturating_sub(alien_line.len()) / 2;
            left_content.push(Line::from(vec![
                Span::raw(" ".repeat(alien_padding)),
                Span::styled(*alien_line, alien_style),
            ]));
        }

        left_content.push(Line::from(vec![Span::raw("")]));

        for info_line in &model_info {
            let info_padding = left_width.saturating_sub(info_line.len()) / 2;
            left_content.push(Line::from(vec![
                Span::raw(" ".repeat(info_padding)),
                Span::styled(*info_line, dim_style),
            ]));
        }

        let left_paragraph = Paragraph::new(left_content);
        frame.render_widget(left_paragraph, left_inner);

        let tips_style = Style::default()
            .fg(accent_color)
            .add_modifier(Modifier::BOLD);

        let separator = "─".repeat(25);
        let separator_style = Style::default()
            .fg(ratatui::style::Color::DarkGray)
            .add_modifier(Modifier::DIM);

        let right_content = vec![
            Line::from(vec![Span::styled("Tips for getting started", tips_style)]),
            Line::from(vec![Span::styled(separator.clone(), separator_style.clone())]),
            Line::from(vec![Span::styled(
                "Run /init to create a CLAUDE.md file with instruc...",
                dim_style,
            )]),
            Line::from(vec![Span::raw("")]),
            Line::from(vec![Span::styled("Recent activity", tips_style)]),
            Line::from(vec![Span::styled(separator, separator_style)]),
            Line::from(vec![Span::styled("No recent activity", dim_style)]),
        ];

        let right_paragraph = Paragraph::new(right_content);
        frame.render_widget(right_paragraph, layout[1]);
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

