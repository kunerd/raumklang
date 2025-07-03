use core::f32;

pub struct Log {
    start_frequency: f32,
    end_frequency: f32,
    sample_rate: usize,
    n_samples: usize,
    amplitude: f32,
}

impl Log {
    pub fn new(
        start_frequency: f32,
        end_frequency: f32,
        amplitude: f32,
        n_samples: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            start_frequency,
            end_frequency,
            sample_rate,
            n_samples,
            amplitude,
        }
    }

    pub fn as_vec(&self) -> Vec<f32> {
        let c = (self.end_frequency / self.start_frequency).ln();

        // get n_samples
        // if sweep_rate is not None:
        //     L = 1 / sweep_rate / np.log(2)
        //     T = L * c
        //     n_samples = np.round(T * sampling_rate)
        // else:
        // n_samples = int(n_samples)

        // L for actual n_samples
        let l = self.n_samples as f32 / self.sample_rate as f32 / c;

        // make the sweep
        // times = np.arange(n_samples) / sampling_rate
        let sweep: Vec<_> = (0..self.n_samples)
            .into_iter()
            .map(|n| n as f32 / self.sample_rate as f32)
            .map(|t| f32::exp(t / l))
            .map(|s| 2.0 * f32::consts::PI * self.start_frequency * l * (s - 1.0))
            .map(|s| self.amplitude * s.sin())
            .collect();

        sweep

        // sweep = amplitude * np.sin(
        //     2 * np.pi * frequency_range[0] * L * (np.exp(times / L) - 1))
    }
}
