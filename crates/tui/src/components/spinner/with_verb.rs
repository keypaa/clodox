use ratatui::layout::Rect;
use ratatui::style::{Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::Widget;
use std::time::{Duration, Instant};

use crate::theme::{Theme, Themeable};
use super::glyph::SpinnerGlyph;
use super::stall_detection::StallDetector;

const TIP_30S: &str = "Use /btw to ask a quick side question without interrupting Claude's current work";
const TIP_30MIN: &str = "Use /clear to start fresh when switching topics and free up context";

const IDLE_PREFIX: &str = "∗";
const IDLE_TEXT: &str = "Idle · teammates running";

static VERBS: &[&str] = &[
    "Thinking",
    "Processing",
    "Working",
    "Analyzing",
    "Generating",
    "Searching",
    "Reading",
    "Writing",
    "Compiling",
    "Loading",
    "Checking",
    "Validating",
    "Formatting",
    "Parsing",
    "Computing",
    "Fetching",
    "Building",
    "Running",
];

pub struct SpinnerWithVerb {
    glyph: SpinnerGlyph,
    stall: StallDetector,
    override_message: Option<String>,
    active_form: Option<String>,
    subject: Option<String>,
    start: Instant,
    has_btw: bool,
}

impl SpinnerWithVerb {
    pub fn new(reduced_motion: bool) -> Self {
        Self {
            glyph: SpinnerGlyph::new(reduced_motion),
            stall: StallDetector::new(),
            override_message: None,
            active_form: None,
            subject: None,
            start: Instant::now(),
            has_btw: false,
        }
    }

    pub fn with_override_message(mut self, msg: String) -> Self {
        self.override_message = Some(msg);
        self
    }

    pub fn with_active_form(mut self, form: String) -> Self {
        self.active_form = Some(form);
        self
    }

    pub fn with_subject(mut self, subject: String) -> Self {
        self.subject = Some(subject);
        self
    }

    pub fn with_btw(mut self, has_btw: bool) -> Self {
        self.has_btw = has_btw;
        self
    }

    pub fn tick(&mut self) {
        self.glyph.tick();
        self.stall.update();
    }

    pub fn record_activity(&mut self) {
        self.stall.record_activity();
    }

    fn selected_verb(&self) -> &str {
        if let Some(msg) = &self.override_message {
            return msg;
        }
        if let Some(form) = &self.active_form {
            return form;
        }
        if let Some(subject) = &self.subject {
            return subject;
        }

        let elapsed_secs = self.start.elapsed().as_secs();
        let idx = (elapsed_secs as usize) % VERBS.len();
        VERBS[idx]
    }

    fn tip_text(&self) -> Option<&str> {
        let elapsed = self.start.elapsed();

        if elapsed > Duration::from_secs(30 * 60) {
            return Some(TIP_30MIN);
        }

        if elapsed > Duration::from_secs(30) && !self.has_btw {
            return Some(TIP_30S);
        }

        None
    }
}

impl Themeable for SpinnerWithVerb {
    fn render_themed(
        &self,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &Theme,
    ) {
        let verb = self.selected_verb();
        let base_color = theme.color("text");
        let color = self.stall.blend_color(base_color);

        let glyph_char = self.glyph.current();
        let glyph_style = Style::default().fg(color);

        let verb_style = Style::default()
            .fg(color)
            .add_modifier(Modifier::DIM);

        let mut spans = vec![
            Span::styled(format!("{}", glyph_char), glyph_style),
            Span::raw(" "),
            Span::styled(format!("{}…", verb), verb_style),
        ];

        if let Some(tip) = self.tip_text() {
            let tip_style = Style::default()
                .fg(theme.color("inactive"))
                .add_modifier(Modifier::DIM);
            spans.push(Span::raw("  "));
            spans.push(Span::styled(format!("> {}", tip), tip_style));
        }

        let line = Line::from(spans);
        line.render_themed(area, buf, theme);
    }
}

impl Widget for SpinnerWithVerb {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let theme = Theme::dark();
        self.render_themed(area, buf, &theme);
    }
}

pub struct IdleStatus {
    reduced_motion: bool,
}

impl IdleStatus {
    pub fn new(reduced_motion: bool) -> Self {
        Self { reduced_motion }
    }
}

impl Themeable for IdleStatus {
    fn render_themed(
        &self,
        area: Rect,
        buf: &mut ratatui::buffer::Buffer,
        theme: &Theme,
    ) {
        let style = Style::default()
            .fg(theme.color("inactive"))
            .add_modifier(Modifier::DIM);

        let prefix_style = if self.reduced_motion {
            style
        } else {
            Style::default()
                .fg(theme.color("inactive"))
                .add_modifier(Modifier::DIM)
        };

        let line = Line::from(vec![
            Span::styled(format!("{} ", IDLE_PREFIX), prefix_style),
            Span::styled(IDLE_TEXT, style),
        ]);
        line.render_themed(area, buf, theme);
    }
}

impl Widget for IdleStatus {
    fn render(self, area: Rect, buf: &mut ratatui::buffer::Buffer) {
        let theme = Theme::dark();
        self.render_themed(area, buf, &theme);
    }
}
