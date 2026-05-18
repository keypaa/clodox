use std::time::{Duration, Instant};

const BRAILLE_FORWARD: &[char] = &['⠋', '⠙', '⠹', '⠸', '⠼', '⠴', '⠦', '⠧', '⠇', '⠏'];
const BRAILLE_REVERSE: &[char] = &['⠏', '⠇', '⠧', '⠦', '⠴', '⠼', '⠸', '⠹', '⠙', '⠋'];
const CYCLE_INTERVAL: Duration = Duration::from_millis(120);

pub struct SpinnerGlyph {
    start: Instant,
    reduced_motion: bool,
}

impl SpinnerGlyph {
    pub fn new(reduced_motion: bool) -> Self {
        Self {
            start: Instant::now(),
            reduced_motion,
        }
    }

    pub fn tick(&mut self) {
        self.start = Instant::now();
    }

    pub fn current(&self) -> char {
        if self.reduced_motion {
            return '●';
        }

        let elapsed = self.start.elapsed();
        let cycle_count = (elapsed.as_millis() / CYCLE_INTERVAL.as_millis()) as usize;
        let full_cycle = BRAILLE_FORWARD.len() + BRAILLE_REVERSE.len() - 2;
        let pos = cycle_count % full_cycle;

        if pos < BRAILLE_FORWARD.len() {
            BRAILLE_FORWARD[pos]
        } else {
            BRAILLE_REVERSE[pos - BRAILLE_FORWARD.len()]
        }
    }

    pub fn reduced_motion_glyph(&self, visible: bool) -> char {
        if visible {
            '●'
        } else {
            '○'
        }
    }

    pub fn is_reduced_motion(&self) -> bool {
        self.reduced_motion
    }
}
