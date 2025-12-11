use std::{
    ops::{Add, AddAssign, Div, Sub},
    time::Duration,
};

use super::SampleRate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct Samples(usize, SampleRate);

impl Samples {
    pub fn new(num: usize, sample_rate: SampleRate) -> Self {
        Self(num, sample_rate)
    }

    pub fn from_duration(duration: Duration, sample_rate: SampleRate) -> Self {
        sample_rate * duration
    }

    pub fn from_f32(samples: f32, sample_rate: SampleRate) -> Self {
        let samples = samples.round() as usize;
        Self(samples, sample_rate)
    }
}

impl From<Samples> for f32 {
    fn from(samples: Samples) -> Self {
        samples.0 as f32
    }
}

impl From<Samples> for usize {
    fn from(samples: Samples) -> Self {
        samples.0
    }
}

impl From<Samples> for Duration {
    fn from(samples: Samples) -> Self {
        let sample_rate: f32 = samples.1.into();
        Duration::from_millis((samples.0 as f32 / sample_rate * 1000.0) as u64)
    }
}

impl AddAssign for Samples {
    fn add_assign(&mut self, rhs: Self) {
        assert!(self.1 == rhs.1);
        self.0 += rhs.0;
    }
}

impl Add for Samples {
    type Output = Samples;

    fn add(self, rhs: Self) -> Self::Output {
        Self(self.0 + rhs.0, self.1)
    }
}

impl Sub for Samples {
    type Output = Samples;

    fn sub(self, rhs: Self) -> Self::Output {
        Self(self.0.saturating_sub(rhs.0), self.1)
    }
}

impl Div for Samples {
    type Output = Samples;

    fn div(self, rhs: Self) -> Self::Output {
        Self(self.0 / rhs.0, self.1)
    }
}

#[cfg(test)]
mod test {
    use std::time::Duration;

    use crate::data::SampleRate;

    use super::Samples;

    #[test]
    fn from_duration() {
        let sample_rate = SampleRate::new(44_100);
        let duration = Duration::from_secs(1);

        let samples = Samples::from_duration(duration, sample_rate);

        assert_eq!(samples.0, 44_100)
    }

    #[test]
    fn into_duration() {
        let sample_rate = SampleRate::new(44_100);

        let duration = Duration::from_secs(1);
        let samples = Samples::from_duration(duration, sample_rate);
        assert_eq!(duration, Duration::from(samples));

        let duration = Duration::from_millis(125);
        let samples = Samples::from_duration(duration, sample_rate);
        assert_eq!(duration, Duration::from(samples));
    }

    #[test]
    fn add_assign() {
        let sample_rate = SampleRate::default();

        let mut samples = Samples::new(1000, sample_rate);

        let rhs = Samples::new(234, sample_rate);
        samples += rhs;

        assert_eq!(samples, Samples::new(1234, sample_rate));
    }

    #[test]
    fn sub() {
        let sample_rate = SampleRate::default();

        let samples = Samples::new(1234, sample_rate);
        let rhs = Samples::new(234, sample_rate);

        let result = samples - rhs;

        assert_eq!(result, Samples::new(1000, sample_rate));
    }

    #[test]
    fn sub_overflow() {
        let sample_rate = SampleRate::default();

        let samples = Samples::new(12, sample_rate);
        let rhs = Samples::new(24, sample_rate);

        let result = samples - rhs;

        assert_eq!(result, Samples::new(0, sample_rate));
    }

    #[test]
    fn from_f32() {
        let sample_rate = SampleRate::default();

        let samples = Samples::from_f32(-1.0, sample_rate);
        assert_eq!(samples, Samples::new(0, sample_rate));

        let samples = Samples::from_f32(0.0, sample_rate);
        assert_eq!(samples, Samples::new(0, sample_rate));

        let samples = Samples::from_f32(1.0, sample_rate);
        assert_eq!(samples, Samples::new(1, sample_rate));

        let samples = Samples::from_f32(1.4, sample_rate);
        assert_eq!(samples, Samples::new(1, sample_rate));

        let samples = Samples::from_f32(1.5, sample_rate);
        assert_eq!(samples, Samples::new(2, sample_rate));
    }
}
