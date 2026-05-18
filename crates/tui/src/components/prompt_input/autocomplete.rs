use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;

use crate::theme::{Theme, Themeable};

#[derive(Debug, Clone)]
pub struct SlashCommand {
    pub name: String,
    pub description: String,
    pub args: String,
}

impl SlashCommand {
    pub fn new(name: &str, description: &str, args: &str) -> Self {
        Self {
            name: name.to_string(),
            description: description.to_string(),
            args: args.to_string(),
        }
    }
}

pub fn builtin_commands() -> Vec<SlashCommand> {
    vec![
        SlashCommand::new("/help", "Show help and available commands", ""),
        SlashCommand::new("/clear", "Clear the conversation history", ""),
        SlashCommand::new("/compact", "Compact the conversation context", ""),
        SlashCommand::new("/quit", "Exit the application", ""),
        SlashCommand::new("/exit", "Exit the application", ""),
        SlashCommand::new("/editor", "Open external editor for input", ""),
        SlashCommand::new("/cost", "Show token usage and cost", ""),
        SlashCommand::new("/model", "Switch the current model", "[model]"),
        SlashCommand::new("/theme", "Change the color theme", "[dark|light|auto]"),
        SlashCommand::new("/transcript", "Toggle transcript mode", ""),
        SlashCommand::new("/verbose", "Toggle verbose output", ""),
        SlashCommand::new("/permissions", "Show or change permission settings", ""),
        SlashCommand::new("/bug", "Report a bug", ""),
        SlashCommand::new("/feedback", "Send feedback", ""),
        SlashCommand::new("/mcp", "Manage MCP servers", "[status|help]"),
        SlashCommand::new("/hooks", "Manage hooks", "[status|help]"),
        SlashCommand::new("/output", "Set output style", "[default|minimal]"),
        SlashCommand::new("/speakeasy", "Toggle voice mode", ""),
        SlashCommand::new("/shift", "Switch project context", "[path]"),
        SlashCommand::new("/btw", "Ask a side question without interrupting", "[question]"),
        SlashCommand::new("/diff", "Show changes made", ""),
        SlashCommand::new("/changes", "Show pending changes", ""),
    ]
}

#[derive(Debug, Clone)]
pub struct AutocompleteState {
    pub active: bool,
    pub matches: Vec<SlashCommand>,
    pub selected_index: usize,
    pub prefix: String,
}

impl AutocompleteState {
    pub fn new() -> Self {
        Self {
            active: false,
            matches: Vec::new(),
            selected_index: 0,
            prefix: String::new(),
        }
    }

    pub fn update(&mut self, input: &str, commands: &[SlashCommand]) {
        if input.starts_with('/') {
            self.prefix = input.to_string();
            self.matches = commands
                .iter()
                .filter(|cmd| cmd.name.starts_with(input))
                .cloned()
                .collect();
            self.active = !self.matches.is_empty();
            self.selected_index = 0;
        } else {
            self.active = false;
            self.matches.clear();
            self.prefix.clear();
        }
    }

    pub fn select_next(&mut self) {
        if !self.matches.is_empty() {
            self.selected_index = (self.selected_index + 1) % self.matches.len();
        }
    }

    pub fn select_prev(&mut self) {
        if !self.matches.is_empty() {
            if self.selected_index == 0 {
                self.selected_index = self.matches.len() - 1;
            } else {
                self.selected_index -= 1;
            }
        }
    }

    pub fn selected(&self) -> Option<&SlashCommand> {
        if self.active && !self.matches.is_empty() {
            self.matches.get(self.selected_index)
        } else {
            None
        }
    }

    pub fn accept(&mut self) -> Option<String> {
        if let Some(cmd) = self.selected() {
            let result = cmd.name.clone();
            self.active = false;
            self.matches.clear();
            Some(result)
        } else {
            None
        }
    }

    pub fn dismiss(&mut self) {
        self.active = false;
        self.matches.clear();
    }

    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Default for AutocompleteState {
    fn default() -> Self {
        Self::new()
    }
}

pub struct AutocompleteWidget<'a> {
    state: &'a AutocompleteState,
    theme: &'a Theme,
}

impl<'a> AutocompleteWidget<'a> {
    pub fn new(state: &'a AutocompleteState, theme: &'a Theme) -> Self {
        Self { state, theme }
    }

    fn render_lines(&self) -> Vec<Line<'static>> {
        if !self.state.active || self.state.matches.is_empty() {
            return vec![];
        }

        let mut lines = Vec::new();
        let max_show = 5;
        let to_show = self.state.matches.iter().take(max_show);

        for (i, cmd) in to_show.enumerate() {
            let is_selected = i == self.state.selected_index;

            let name_style = if is_selected {
                Style::default()
                    .fg(self.theme.colors.suggestion)
                    .add_modifier(Modifier::BOLD)
            } else {
                Style::default().fg(self.theme.colors.suggestion)
            };

            let desc_style = if is_selected {
                Style::default()
                    .fg(self.theme.colors.text)
                    .add_modifier(Modifier::DIM)
            } else {
                Style::default()
                    .fg(self.theme.colors.inactive)
                    .add_modifier(Modifier::DIM)
            };

            let mut spans = vec![Span::styled(cmd.name.clone(), name_style)];

            if !cmd.args.is_empty() {
                spans.push(Span::styled(
                    format!(" {}", cmd.args),
                    Style::default()
                        .fg(self.theme.colors.inactive)
                        .add_modifier(Modifier::DIM),
                ));
            }

            spans.push(Span::raw("  "));
            spans.push(Span::styled(cmd.description.clone(), desc_style));

            lines.push(Line::from(spans));
        }

        lines
    }
}

impl Themeable for AutocompleteWidget<'_> {
    fn render_themed(&self, area: Rect, buf: &mut ratatui::buffer::Buffer, theme: &Theme) {
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

impl Widget for AutocompleteWidget<'_> {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        self.render_themed(area, buf, &Theme::dark());
    }
}
