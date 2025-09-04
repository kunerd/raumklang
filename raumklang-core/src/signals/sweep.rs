mod exponential;

use core::f32;
use std::time::Duration;

pub use exponential::ExponentialSweep;

#[derive(Debug, Clone)]
pub struct LinearSineSweep {
    sample_rate: usize,
    sample_index: usize,
    n_samples: usize,
    amplitude: f32,
    frequency: f32,
    delta_frequency: f32,
    phase: f32,
}

// linear sweep
impl LinearSineSweep {
    pub fn new(
        start_frequency: u16,
        end_frequency: u16,
        duration: Duration,
        amplitude: f32,
        sample_rate: usize,
    ) -> Self {
        let n_samples = sample_rate as f32 * duration.as_secs_f32();
        LinearSineSweep {
            sample_rate,
            sample_index: 0,
            n_samples: n_samples as usize,
            amplitude,
            frequency: start_frequency as f32,
            delta_frequency: (end_frequency - start_frequency) as f32 / n_samples,
            phase: 0.0,
        }
    }
}

impl Iterator for LinearSineSweep {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.sample_index < self.n_samples {
            true => Some(self.amplitude * f32::sin(self.phase)),
            _ => None,
        };
        self.frequency += self.delta_frequency;
        let delta_phase = 2.0 * std::f32::consts::PI * self.frequency / self.sample_rate as f32;
        self.phase = (self.phase + delta_phase) % (2.0 * std::f32::consts::PI);
        //let frequency = f32::exp(
        //    f32::ln(self.start_frequency) * (1.0 - self.sample_index / self.duration)
        //        + f32::ln(self.end_frequency) * (self.sample_index / self.duration),
        //);

        self.sample_index += 1;

        result
    }
}

impl ExactSizeIterator for LinearSineSweep {
    fn len(&self) -> usize {
        self.n_samples - self.sample_index
    }
}
