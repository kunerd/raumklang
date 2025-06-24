use std::{fmt::Display, ops::Mul, time::Duration};

use super::Samples;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct SampleRate(u32);

impl SampleRate {
    pub fn new(sample_rate: u32) -> Self {
        Self(sample_rate)
    }
}

impl Mul<Duration> for SampleRate {
    type Output = Samples;

    fn mul(self, rhs: Duration) -> Self::Output {
        let samples = rhs.as_secs_f32() * self.0 as f32;

        Samples::from_f32(samples, self)
    }
}

impl Mul<SampleRate> for Duration {
    type Output = Samples;

    fn mul(self, rhs: SampleRate) -> Self::Output {
        let samples = self.as_secs_f32() * rhs.0 as f32;

        Samples::from_f32(samples, rhs)
    }
}

impl From<SampleRate> for f32 {
    fn from(value: SampleRate) -> Self {
        value.0 as f32
    }
}

impl From<SampleRate> for u32 {
    fn from(value: SampleRate) -> Self {
        value.0
    }
}

impl From<usize> for SampleRate {
    fn from(value: usize) -> Self {
        Self(value as u32)
    }
}

impl From<u32> for SampleRate {
    fn from(value: u32) -> Self {
        Self(value)
    }
}

impl Default for SampleRate {
    fn default() -> Self {
        Self(44_100)
    }
}

impl Display for SampleRate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[cfg(test)]
mod test {
    use super::*;

    #[test]
    fn multiply_sample_rate_with_duration() {
        let sample_rate = SampleRate::new(44_100);
        let duration = Duration::from_secs(2);

        let result = sample_rate * duration;

        assert_eq!(result, Samples::new(88_200, sample_rate))
    }

    #[test]
    fn multiply_duration_with_sample_rate() {
        let sample_rate = SampleRate::new(44_100);
        let duration = Duration::from_secs(2);

        let result = duration * sample_rate;

        assert_eq!(result, Samples::new(88_200, sample_rate))
    }
}
