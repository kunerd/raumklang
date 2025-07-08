use rand::{distributions, distributions::Distribution, rngs, SeedableRng};

#[derive(Debug, Clone)]
pub struct WhiteNoise {
    amplitude: f32,
    rng: rngs::SmallRng,
    distribution: distributions::Uniform<f32>,
}

impl WhiteNoise {
    pub fn with_amplitude(amplitude: f32) -> Self {
        WhiteNoise {
            amplitude,
            rng: rngs::SmallRng::from_entropy(),
            distribution: distributions::Uniform::new_inclusive(-1.0, 1.0),
        }
    }

    pub fn take_duration(self, sample_rate: usize, duration: usize) -> std::iter::Take<WhiteNoise> {
        self.into_iter().take(sample_rate * duration)
    }
}

impl Default for WhiteNoise {
    fn default() -> Self {
        Self::with_amplitude(1.0)
    }
}

impl Iterator for WhiteNoise {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.amplitude * self.distribution.sample(&mut self.rng);
        Some(sample)
    }
}

impl ExactSizeIterator for WhiteNoise {}

#[derive(Debug, Clone)]
pub struct PinkNoise {
    b0: f32,
    b1: f32,
    b2: f32,
    white_noise: WhiteNoise,
}

impl PinkNoise {
    pub fn with_amplitude(amplitude: f32) -> Self {
        let white_noise = WhiteNoise::with_amplitude(amplitude);

        PinkNoise {
            b0: 0f32,
            b1: 0f32,
            b2: 0f32,
            white_noise,
        }
    }

    pub fn take_duration(self, sample_rate: usize, duration: usize) -> std::iter::Take<PinkNoise> {
        self.into_iter().take(sample_rate * duration)
    }
}

impl Default for PinkNoise {
    fn default() -> Self {
        Self::with_amplitude(1.0)
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

impl ExactSizeIterator for PinkNoise {}
