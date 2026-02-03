use raumklang_core::{dbfs, loudness};

use tokio::sync::mpsc::error::TrySendError;

use std::time::{Duration, Instant};

use super::{Process, process::Control};

#[derive(Debug, Clone, Copy)]
pub struct Loudness {
    pub rms: f32,
    pub peak: f32,
}

impl Default for Loudness {
    fn default() -> Self {
        Self {
            rms: f32::NEG_INFINITY,
            peak: f32::NEG_INFINITY,
        }
    }
}

pub struct Test {
    last_rms: Instant,
    last_peak: Instant,
    meter: loudness::Meter,
    sender: tokio::sync::mpsc::Sender<Loudness>,
}

impl Test {
    pub fn new(sender: tokio::sync::mpsc::Sender<Loudness>) -> Self {
        let last_rms = Instant::now();
        let last_peak = Instant::now();

        // FIXME hardcoded sample rate dependency
        let meter = loudness::Meter::new(13230); // 44100samples / 1000ms * 300ms

        Self {
            last_rms,
            last_peak,
            meter,
            sender,
        }
    }
}

impl Process for Test {
    fn process(&mut self, data: &[f32]) -> Control {
        self.meter.update_from_iter(data.iter().copied());

        if self.last_rms.elapsed() > Duration::from_millis(150) {
            let loudness = Loudness {
                rms: dbfs(self.meter.rms()),
                peak: dbfs(self.meter.peak()),
            };

            match self.sender.try_send(loudness) {
                Ok(_) => {}
                Err(TrySendError::Full(_)) => {}
                Err(TrySendError::Closed(_)) => {
                    return Control::Stop;
                }
            }

            self.last_rms = Instant::now();
        }

        if self.last_peak.elapsed() > Duration::from_millis(500) {
            self.meter.reset_peak();
            self.last_peak = Instant::now();
        }

        Control::Continue
    }
}
