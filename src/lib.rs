pub struct SineSweep {
    sample_rate: u32,
    sample_index: u32,
    amplitude: f32,
    frequency: f32,
    delta_frequency: f32,
    phase: f32,
    duration: u32,
}

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
            delta_frequency: (end_frequency - start_frequency) as f32 / (sample_rate * duration) as f32,
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
