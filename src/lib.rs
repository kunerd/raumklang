use rand::{distributions, distributions::Distribution, rngs, Rng, SeedableRng};

pub struct SineSweep {
    sample_rate: u32,
    sample_index: u32,
    amplitude: f32,
    frequency: f32,
    delta_frequency: f32,
    phase: f32,
    duration: u32,
}

// linear sweep
// TODO: implement log sweep
impl SineSweep {
    pub fn new(
        start_frequency: u32,
        end_frequency: u32,
        duration: u32,
        amplitude: f32,
        sample_rate: u32,
    ) -> Self {
        SineSweep {
            sample_rate,
            sample_index: 0,
            amplitude,
            frequency: start_frequency as f32,
            delta_frequency: (end_frequency - start_frequency) as f32
                / (sample_rate * duration) as f32,
            phase: 0.0,
            duration,
        }
    }
}

impl Iterator for SineSweep {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let n_samples = self.sample_rate * self.duration;
        let result = match self.sample_index < n_samples {
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

pub struct WhiteNoise {
    amplitude: f32,
    rng: rngs::SmallRng,
    distribution: distributions::Uniform<f32>,
}

impl WhiteNoise {
    pub fn new() -> Self {
        Self::with_amplitude(0.5)
    }

    pub fn with_amplitude(amplitude: f32) -> Self {
        WhiteNoise {
            amplitude,
            rng: rngs::SmallRng::from_entropy(),
            distribution: distributions::Uniform::new_inclusive(-1.0, 1.0),
        }
    }
}

impl Iterator for WhiteNoise {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.amplitude * self.distribution.sample(&mut self.rng);
        Some(sample)
    }
}

pub struct PinkNoise {
    b0: f32,
    b1: f32,
    b2: f32,
    white_noise: WhiteNoise,
}

impl PinkNoise {
    pub fn new() -> Self {
        Self::with_amplitude(0.5)
    }

    pub fn with_amplitude(amplitude: f32) -> Self {
        let mut white_noise = WhiteNoise::with_amplitude(amplitude);
        let white = white_noise.next().unwrap();

        PinkNoise {
            b0: 0f32,
            b1: 0f32,
            b2: 0f32,
            white_noise,
        }
    }
}

impl Iterator for PinkNoise {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let white = self.white_noise.next().unwrap();
        self.b0 = 0.99765 * self.b0 + white * 0.0990460;
        self.b1 = 0.96300 * self.b1 + white * 0.2965164;
        self.b2 = 0.57000 * self.b2 + white * 1.0526913;
        Some(self.b0 + self.b1 + self.b2 + white * 0.1848)
    }
}
