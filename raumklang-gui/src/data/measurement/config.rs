use std::{num::ParseIntError, time::Duration};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    duration: Duration,
    frequency_range: FrequencyRange,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FrequencyRange {
    from: u16,
    to: u16,
}

#[derive(Debug, thiserror::Error)]
pub enum Error<'a> {
    #[error("parsing {field:?} failed: {err:?}")]
    ParseField { field: &'a str, err: ParseIntError },
    #[error("`from` must be lesser than `to`")]
    Range,
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
