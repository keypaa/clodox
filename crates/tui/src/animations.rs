use std::time::{Duration, Instant};

const DEFAULT_FPS_TARGET: u32 = 30;
const FPS_SAMPLE_SIZE: usize = 60;
const SHIMMER_FAST_INTERVAL: Duration = Duration::from_millis(50);
const SHIMMER_SLOW_INTERVAL: Duration = Duration::from_millis(200);
const SMOOTH_INTERPOLATION_SPEED: f32 = 0.15;
const REDUCED_MOTION_THRESHOLD: Duration = Duration::from_millis(100);

#[derive(Debug, Clone, Copy)]
pub struct AnimationConfig {
    pub reduced_motion: bool,
    pub spinner_interval: Duration,
    pub shimmer_interval: Duration,
    pub stall_factor: f32,
    pub smooth_speed: f32,
}

impl Default for AnimationConfig {
    fn default() -> Self {
        Self {
            reduced_motion: false,
            spinner_interval: Duration::from_millis(120),
            shimmer_interval: SHIMMER_SLOW_INTERVAL,
            stall_factor: 0.1,
            smooth_speed: SMOOTH_INTERPOLATION_SPEED,
        }
    }
}

impl AnimationConfig {
    pub fn reduced_motion() -> Self {
        Self {
            reduced_motion: true,
            spinner_interval: Duration::from_secs(2),
            shimmer_interval: Duration::from_secs(1),
            stall_factor: 0.0,
            smooth_speed: 1.0,
        }
    }

    pub fn with_reduced_motion(mut self, enabled: bool) -> Self {
        self.reduced_motion = enabled;
        if enabled {
            self.spinner_interval = Duration::from_secs(2);
            self.shimmer_interval = Duration::from_secs(1);
            self.stall_factor = 0.0;
            self.smooth_speed = 1.0;
        } else {
            self.spinner_interval = Duration::from_millis(120);
            self.shimmer_interval = SHIMMER_SLOW_INTERVAL;
            self.stall_factor = 0.1;
            self.smooth_speed = SMOOTH_INTERPOLATION_SPEED;
        }
        self
    }
}

#[derive(Debug, Clone)]
pub struct AnimationTicker {
    config: AnimationConfig,
    last_frame: Instant,
    frame_count: u64,
    fps_samples: Vec<Duration>,
    current_fps: u32,
    elapsed: Duration,
    frame_index: u64,
}

impl AnimationTicker {
    pub fn new(config: AnimationConfig) -> Self {
        Self {
            config,
            last_frame: Instant::now(),
            frame_count: 0,
            fps_samples: Vec::with_capacity(FPS_SAMPLE_SIZE),
            current_fps: 0,
            elapsed: Duration::ZERO,
            frame_index: 0,
        }
    }

    pub fn tick(&mut self) -> FrameInfo {
        let now = Instant::now();
        let delta = now.duration_since(self.last_frame);
        self.last_frame = now;
        self.elapsed += delta;
        self.frame_count += 1;
        self.frame_index += 1;

        self.fps_samples.push(delta);
        if self.fps_samples.len() > FPS_SAMPLE_SIZE {
            self.fps_samples.remove(0);
        }

        if !self.fps_samples.is_empty() {
            let total: Duration = self.fps_samples.iter().sum();
            let avg = total / self.fps_samples.len() as u32;
            if avg.as_millis() > 0 {
                self.current_fps = (1000.0 / avg.as_millis() as f64) as u32;
            }
        }

        FrameInfo {
            delta,
            elapsed: self.elapsed,
            frame_index: self.frame_index,
            fps: self.current_fps,
            reduced_motion: self.config.reduced_motion,
        }
    }

    pub fn should_render_spinner(&self, frame: &FrameInfo) -> bool {
        if self.config.reduced_motion {
            let cycle = frame.elapsed.as_secs() % 4;
            return cycle < 2;
        }

        let cycle = frame.elapsed.as_millis() / self.config.spinner_interval.as_millis();
        cycle % 2 == 0
    }

    pub fn should_render_shimmer(&self, frame: &FrameInfo) -> bool {
        if self.config.reduced_motion {
            let cycle = frame.elapsed.as_secs() % 2;
            return cycle == 0;
        }

        let cycle = frame.elapsed.as_millis() / self.config.shimmer_interval.as_millis();
        cycle % 2 == 0
    }

    pub fn shimmer_intensity(&self, frame: &FrameInfo) -> f32 {
        if self.config.reduced_motion {
            return 0.5;
        }

        let interval = self.config.shimmer_interval.as_millis() as f32;
        let phase = (frame.elapsed.as_millis() as f32 % interval) / interval;
        (phase * std::f32::consts::PI * 2.0).sin() * 0.5 + 0.5
    }

    pub fn smooth_value(&self, current: f32, target: f32, delta: Duration) -> f32 {
        if self.config.reduced_motion {
            return target;
        }

        let t = (delta.as_secs_f32() * self.config.smooth_speed * 60.0).min(1.0);
        current + (target - current) * t
    }

    pub fn smooth_token_count(&self, current: u64, target: u64, delta: Duration) -> u64 {
        if self.config.reduced_motion {
            return target;
        }

        let current_f = current as f32;
        let target_f = target as f32;
        let t = (delta.as_secs_f32() * self.config.smooth_speed * 60.0).min(1.0);
        let smoothed = current_f + (target_f - current_f) * t;
        smoothed.round() as u64
    }

    pub fn stall_blend_factor(&self, elapsed_since_activity: Duration, threshold: Duration) -> f32 {
        if self.config.reduced_motion {
            return 0.0;
        }

        if elapsed_since_activity < threshold {
            return 0.0;
        }

        let stall_duration = elapsed_since_activity - threshold;
        let fade_duration = Duration::from_secs(2);
        let factor = (stall_duration.as_millis() as f32 / fade_duration.as_millis() as f32).min(1.0);
        factor * self.config.stall_factor
    }

    pub fn config(&self) -> &AnimationConfig {
        &self.config
    }

    pub fn set_config(&mut self, config: AnimationConfig) {
        self.config = config;
    }

    pub fn fps(&self) -> u32 {
        self.current_fps
    }

    pub fn frame_count(&self) -> u64 {
        self.frame_count
    }

    pub fn elapsed(&self) -> Duration {
        self.elapsed
    }
}

impl Default for AnimationTicker {
    fn default() -> Self {
        Self::new(AnimationConfig::default())
    }
}

#[derive(Debug, Clone, Copy)]
pub struct FrameInfo {
    pub delta: Duration,
    pub elapsed: Duration,
    pub frame_index: u64,
    pub fps: u32,
    pub reduced_motion: bool,
}

pub struct SmoothValue {
    current: f32,
    target: f32,
}

impl SmoothValue {
    pub fn new(initial: f32) -> Self {
        Self {
            current: initial,
            target: initial,
        }
    }

    pub fn set_target(&mut self, target: f32) {
        self.target = target;
    }

    pub fn update(&mut self, ticker: &AnimationTicker, delta: Duration) -> f32 {
        self.current = ticker.smooth_value(self.current, self.target, delta);
        self.current
    }

    pub fn current(&self) -> f32 {
        self.current
    }

    pub fn target(&self) -> f32 {
        self.target
    }
}

pub struct SmoothCounter {
    current: u64,
    target: u64,
}

impl SmoothCounter {
    pub fn new(initial: u64) -> Self {
        Self {
            current: initial,
            target: initial,
        }
    }

    pub fn set_target(&mut self, target: u64) {
        self.target = target;
    }

    pub fn update(&mut self, ticker: &AnimationTicker, delta: Duration) -> u64 {
        self.current = ticker.smooth_token_count(self.current, self.target, delta);
        self.current
    }

    pub fn current(&self) -> u64 {
        self.current
    }

    pub fn target(&self) -> u64 {
        self.target
    }
}
