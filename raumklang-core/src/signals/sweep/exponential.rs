#[derive(Debug, Clone)]
pub struct ExponentialSweep {
    sample_index: usize,
    start_frequency: f32,
    end_frequency: f32,
    sample_rate: usize,
    n_samples: usize,
    amplitude: f32,
}

impl ExponentialSweep {
    pub fn new(
        start_frequency: f32,
        end_frequency: f32,
        amplitude: f32,
        n_samples: usize,
        sample_rate: usize,
    ) -> Self {
        Self {
            sample_index: 0,
            start_frequency,
            end_frequency,
            sample_rate,
            n_samples,
            amplitude,
        }
    }
}

impl Iterator for ExponentialSweep {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        use std::f32::consts::PI;

        if self.sample_index < self.n_samples {
            let c = (self.end_frequency / self.start_frequency).ln();
            let l = self.n_samples as f32 / self.sample_rate as f32 / c;

            let t = self.sample_index as f32 / self.sample_rate as f32;
            let s = 2.0 * PI * self.start_frequency * l * (f32::exp(t / l) - 1.0);
            let s = self.amplitude * f32::sin(s);

            self.sample_index += 1;

            Some(s)
        } else {
            None
        }
    }
}

impl ExactSizeIterator for ExponentialSweep {
    fn len(&self) -> usize {
        let (lower, upper) = (0, Some(self.n_samples));
        // Note: This assertion is overly defensive, but it checks the invariant
        // guaranteed by the trait. If this trait were rust-internal,
        // we could use debug_assert!; assert_eq! will check all Rust user
        // implementations too.
        std::assert_eq!(upper, Some(lower));
        lower
    }
}
