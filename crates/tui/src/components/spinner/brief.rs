use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::time::{Duration, Instant};

use crate::theme::{Theme, Themeable};
use super::glyph::SpinnerGlyph;

const BRIEF_BLINK_INTERVAL: Duration = Duration::from_millis(800);

pub struct BriefSpinner {
    glyph: SpinnerGlyph,
    start: Instant,
}

impl BriefSpinner {
    pub fn new(reduced_motion: bool) -> Self {
        Self {
            glyph: SpinnerGlyph::new(reduced_motion),
            start: Instant::now(),
        }
    }

    pub fn tick(&mut self) {
        self.glyph.tick();
    }

    fn is_visible(&self) -> bool {
        if self.glyph.is_reduced_motion() {
            let elapsed = self.start.elapsed();
            let cycle = elapsed.as_secs() % 4;
            cycle < 2
        } else {
            let elapsed = self.start.elapsed();
            let cycle = (elapsed.as_millis() / BRIEF_BLINK_INTERVAL.as_millis()) % 2;
            cycle == 0
        }
    }
}

impl Themeable for BriefSpinner {
    fn render_themed(
        &self,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &Theme,
    ) {
        let visible = self.is_visible();
        let char = if visible { '●' } else { '○' };

        let style = if visible {
            Style::default()
                .fg(theme.color("inactive"))
                .add_modifier(Modifier::DIM)
        } else {
            Style::default().fg(Color::DarkGray)
        };

        let span = Span::styled(String::from(char), style);
        let line = Line::from(vec![span]);
        line.render_themed(area, buf, theme);
    }
}

impl Widget for BriefSpinner {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let theme = Theme::dark();
        self.render_themed(area, buf, &theme);
    }
}

pub struct BriefIdleStatus {
    reduced_motion: bool,
}

impl BriefIdleStatus {
    pub fn new(reduced_motion: bool) -> Self {
        Self { reduced_motion }
    }
}

impl Themeable for BriefIdleStatus {
    fn render_themed(
        &self,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &Theme,
    ) {
        let style = Style::default()
            .fg(theme.color("inactive"))
            .add_modifier(Modifier::DIM);

        let line = Line::from(vec![Span::styled("∗ idle", style)]);
        line.render_themed(area, buf, theme);
    }
}

impl Widget for BriefIdleStatus {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let theme = Theme::dark();
        self.render_themed(area, buf, &theme);
    }
}
