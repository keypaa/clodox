use ratatui::layout::Rect;
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::time::{Duration, Instant};

use crate::theme::{Theme, Themeable};

const BLINK_INTERVAL: Duration = Duration::from_millis(500);

#[derive(Clone, Copy, PartialEq)]
pub enum SpinnerState {
    Unresolved,
    Resolved,
    Errored,
}

pub struct SpinnerAnimationRow {
    state: SpinnerState,
    start: Instant,
    reduced_motion: bool,
}

impl SpinnerAnimationRow {
    pub fn new(state: SpinnerState, reduced_motion: bool) -> Self {
        Self {
            state,
            start: Instant::now(),
            reduced_motion,
        }
    }

    pub fn with_state(mut self, state: SpinnerState) -> Self {
        self.state = state;
        self
    }

    fn dot_color(&self) -> Color {
        match self.state {
            SpinnerState::Unresolved => Color::Rgb(177, 173, 161),
            SpinnerState::Resolved => Color::Green,
            SpinnerState::Errored => Color::Red,
        }
    }

    fn is_visible(&self) -> bool {
        if self.reduced_motion {
            let elapsed = self.start.elapsed();
            let cycle = elapsed.as_secs() % 4;
            cycle < 2
        } else {
            let elapsed = self.start.elapsed();
            let cycle = (elapsed.as_millis() / BLINK_INTERVAL.as_millis()) % 2;
            cycle == 0
        }
    }
}

impl Themeable for SpinnerAnimationRow {
    fn render_themed(
        &self,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &Theme,
    ) {
        let visible = self.is_visible();
        let dot_char = if self.reduced_motion {
            if visible { '●' } else { '○' }
        } else {
            '●'
        };

        let style = if visible {
            Style::default().fg(self.dot_color())
        } else {
            Style::default()
                .fg(Color::Rgb(177, 173, 161))
                .add_modifier(Modifier::DIM)
        };

        let span = Span::styled(String::from(dot_char), style);
        let line = Line::from(vec![span]);
        line.render_themed(area, buf, theme);
    }
}

impl Widget for SpinnerAnimationRow {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let theme = Theme::dark();
        self.render_themed(area, buf, &theme);
    }
}
