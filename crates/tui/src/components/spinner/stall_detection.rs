use ratatui::style::Color;
use std::time::{Duration, Instant};

const STALL_THRESHOLD: Duration = Duration::from_secs(3);
const STALL_FADE_DURATION: Duration = Duration::from_secs(2);
const STALL_FADE_INTERVAL: Duration = Duration::from_millis(50);
const STALL_FADE_FACTOR: f32 = 0.1;
const ERROR_RED: Color = Color::Rgb(171, 43, 63);

pub struct StallDetector {
    last_activity: Instant,
    stall_start: Option<Instant>,
    current_intensity: f32,
    last_fade_tick: Instant,
}

impl StallDetector {
    pub fn new() -> Self {
        Self {
            last_activity: Instant::now(),
            stall_start: None,
            current_intensity: 0.0,
            last_fade_tick: Instant::now(),
        }
    }

    pub fn record_activity(&mut self) {
        self.last_activity = Instant::now();
        self.stall_start = None;
        self.current_intensity = 0.0;
        self.last_fade_tick = Instant::now();
    }

    pub fn update(&mut self) {
        let elapsed_since_activity = self.last_activity.elapsed();

        if elapsed_since_activity < STALL_THRESHOLD {
            self.stall_start = None;
            self.current_intensity = 0.0;
            return;
        }

        if self.stall_start.is_none() {
            self.stall_start = Some(Instant::now());
            self.current_intensity = 0.0;
        }

        let fade_elapsed = self.last_fade_tick.elapsed();
        if fade_elapsed >= STALL_FADE_INTERVAL {
            self.current_intensity = (self.current_intensity + STALL_FADE_FACTOR).min(1.0);
            self.last_fade_tick = Instant::now();
        }
    }

    pub fn is_stalled(&self) -> bool {
        self.stall_start.is_some()
    }

    pub fn stall_intensity(&self) -> f32 {
        self.current_intensity
    }

    pub fn blend_color(&self, base: Color) -> Color {
        if self.current_intensity <= 0.0 {
            return base;
        }

        let (br, bg, bb) = color_to_rgb(base);
        let (er, eg, eb) = color_to_rgb(ERROR_RED);

        let t = self.current_intensity;
        Color::Rgb(
            (br as f32 * (1.0 - t) + er as f32 * t) as u8,
            (bg as f32 * (1.0 - t) + eg as f32 * t) as u8,
            (bb as f32 * (1.0 - t) + eb as f32 * t) as u8,
        )
    }

    pub fn time_since_activity(&self) -> Duration {
        self.last_activity.elapsed()
    }
}

fn color_to_rgb(color: Color) -> (u8, u8, u8) {
    match color {
        Color::Rgb(r, g, b) => (r, g, b),
        Color::Black => (0, 0, 0),
        Color::Red => (255, 0, 0),
        Color::Green => (0, 255, 0),
        Color::Yellow => (255, 255, 0),
        Color::Blue => (0, 0, 255),
        Color::Magenta => (255, 0, 255),
        Color::Cyan => (0, 255, 255),
        Color::White => (255, 255, 255),
        Color::DarkGray => (128, 128, 128),
        Color::LightRed => (255, 128, 128),
        Color::LightGreen => (128, 255, 128),
        Color::LightYellow => (255, 255, 128),
        Color::LightBlue => (128, 128, 255),
        Color::LightMagenta => (255, 128, 255),
        Color::LightCyan => (128, 255, 255),
        Color::Gray => (192, 192, 192),
        _ => (128, 128, 128),
    }
}
