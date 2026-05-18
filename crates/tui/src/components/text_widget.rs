use ratatui::{
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::Paragraph,
};

/// Text widget — styled text matching Ink's Text component.
///
/// Supports: color, bold, dim, italic, underline, inverse, wrap, truncate.
pub struct TextWidget {
    pub content: String,
    pub color: Option<Color>,
    pub background: Option<Color>,
    pub bold: bool,
    pub dim: bool,
    pub italic: bool,
    pub underline: bool,
    pub inverse: bool,
    pub wrap: bool,
    pub truncate_end: bool,
}

impl TextWidget {
    pub fn new(content: impl Into<String>) -> Self {
        Self {
            content: content.into(),
            color: None,
            background: None,
            bold: false,
            dim: false,
            italic: false,
            underline: false,
            inverse: false,
            wrap: false,
            truncate_end: false,
        }
    }

    pub fn color(mut self, color: Color) -> Self {
        self.color = Some(color);
        self
    }

    pub fn background(mut self, color: Color) -> Self {
        self.background = Some(color);
        self
    }

    pub fn bold(mut self) -> Self {
        self.bold = true;
        self
    }

    pub fn dim(mut self) -> Self {
        self.dim = true;
        self
    }

    pub fn italic(mut self) -> Self {
        self.italic = true;
        self
    }

    pub fn underline(mut self) -> Self {
        self.underline = true;
        self
    }

    pub fn inverse(mut self) -> Self {
        self.inverse = true;
        self
    }

    pub fn wrap(mut self) -> Self {
        self.wrap = true;
        self
    }

    pub fn truncate_end(mut self) -> Self {
        self.truncate_end = true;
        self
    }

    /// Build the ratatui Style from the widget properties.
    pub fn style(&self) -> Style {
        let mut style = Style::default();

        if let Some(color) = self.color {
            style = style.fg(color);
        }
        if let Some(color) = self.background {
            style = style.bg(color);
        }

        let mut modifiers = Modifier::empty();
        if self.bold {
            modifiers |= Modifier::BOLD;
        }
        if self.dim {
            modifiers |= Modifier::DIM;
        }
        if self.italic {
            modifiers |= Modifier::ITALIC;
        }
        if self.underline {
            modifiers |= Modifier::UNDERLINED;
        }
        if self.inverse {
            modifiers |= Modifier::REVERSED;
        }
        style = style.add_modifier(modifiers);

        style
    }

    /// Convert to a ratatui Span.
    pub fn to_span(&self) -> Span {
        Span::styled(&self.content, self.style())
    }

    /// Convert to a ratatui Line.
    pub fn to_line(&self) -> Line {
        Line::from(self.to_span())
    }
}

impl ratatui::widgets::Widget for TextWidget {
    fn render(self, area: ratatui::layout::Rect, buf: &mut ratatui::buffer::Buffer) {
        let mut paragraph = Paragraph::new(self.to_line()).style(self.style());

        if self.wrap {
            paragraph = paragraph.wrap(ratatui::widgets::Wrap { trim: false });
        }
        if self.truncate_end {
            // Truncate is handled by the content itself
            let max_width = area.width as usize;
            if self.content.len() > max_width {
                let truncated = self
                    .content
                    .chars()
                    .take(max_width.saturating_sub(1))
                    .collect::<String>()
                    + "…";
                paragraph = Paragraph::new(Span::styled(truncated, self.style()));
            }
        }

        paragraph.render(area, buf);
    }
}

/// Create a dim text widget.
pub fn dim_text(content: impl Into<String>) -> TextWidget {
    TextWidget::new(content).dim()
}

/// Create a bold text widget.
pub fn bold_text(content: impl Into<String>) -> TextWidget {
    TextWidget::new(content).bold()
}

/// Create an error text widget.
pub fn error_text(content: impl Into<String>) -> TextWidget {
    TextWidget::new(content).color(Color::Red)
}

/// Create a suggestion/highlight text widget.
pub fn suggestion_text(content: impl Into<String>) -> TextWidget {
    TextWidget::new(content).color(Color::Blue)
}

/// Create a success text widget.
pub fn success_text(content: impl Into<String>) -> TextWidget {
    TextWidget::new(content).color(Color::Green)
}
