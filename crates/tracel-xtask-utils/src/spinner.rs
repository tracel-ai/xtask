use std::time::{Duration, Instant};

const DEFAULT_FRAMES: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

pub const CLR_EOL: &str = "\x1b[K";

#[derive(Debug, Clone)]
pub struct Spinner {
    frames: &'static [&'static str],
    frame_index: usize,
    start: Instant,
}

impl Default for Spinner {
    fn default() -> Self {
        Self::new()
    }
}

impl Spinner {
    pub fn new() -> Self {
        Self {
            frames: &DEFAULT_FRAMES,
            frame_index: 0,
            start: Instant::now(),
        }
    }

    pub fn with_frames(frames: &'static [&'static str]) -> Self {
        Self {
            frames,
            frame_index: 0,
            start: Instant::now(),
        }
    }

    pub fn next_frame(&mut self) -> &'static str {
        let frame = self.frames[self.frame_index % self.frames.len()];
        self.frame_index += 1;
        frame
    }

    pub fn elapsed(&self) -> Duration {
        self.start.elapsed()
    }

    pub fn elapsed_mm_ss(&self) -> (u64, u64) {
        let elapsed_secs = self.elapsed().as_secs();
        (elapsed_secs / 60, elapsed_secs % 60)
    }

    pub fn restart(&mut self) {
        self.start = Instant::now();
        self.frame_index = 0;
    }
}
