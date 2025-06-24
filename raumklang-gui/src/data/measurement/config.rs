use std::{
    num::{ParseFloatError, ParseIntError},
    time,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    frequency_range: FrequencyRange,
    duration: Duration,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrequencyRange {
    from: u16,
    to: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Duration(time::Duration);

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error<'a> {
    #[error("parsing '{field}' failed: {err}")]
    ParseField { field: &'a str, err: ParseIntError },
    #[error("`from` must be lesser than `to`")]
    Range,
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("needs to be greater than zero")]
    SmallerThanZero,
    #[error("needs to be floating point")]
    Parse(#[from] ParseFloatError),
}

impl FrequencyRange {
    pub fn from_strings<'a, 'b>(from: &'a str, to: &'a str) -> Result<Self, Error<'b>> {
        let from = from
            .parse()
            .map_err(|err| Error::ParseField { field: "from", err })?;
        let to = to
            .parse()
            .map_err(|err| Error::ParseField { field: "to", err })?;

        if from < to {
            Ok(Self { from, to })
        } else {
            Err(Error::Range)
        }
    }
}

impl Duration {
    pub fn from_string(duration: &str) -> Result<Self, ValidationError> {
        let duration = duration.parse()?;

        if duration <= 0.0 {
            return Err(ValidationError::SmallerThanZero);
        }

        let duration = time::Duration::from_secs_f32(duration);

        Ok(Self(duration))
    }

    pub fn into_inner(self) -> time::Duration {
        self.0
    }

    fn from_secs(secs: u64) -> Duration {
        Duration(time::Duration::from_secs(secs))
    }
}

impl Config {
    pub fn new(frequency_range: FrequencyRange, duration: Duration) -> Self {
        Self {
            duration,
            frequency_range,
        }
    }

    pub fn duration(&self) -> Duration {
        self.duration
    }

    pub fn start_frequency(&self) -> u16 {
        self.frequency_range.from
    }

    pub fn end_frequency(&self) -> u16 {
        self.frequency_range.to
    }
}

impl Default for FrequencyRange {
    fn default() -> Self {
        Self {
            from: 20,
            to: 20_000,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(5),
            frequency_range: FrequencyRange::default(),
        }
    }
}
