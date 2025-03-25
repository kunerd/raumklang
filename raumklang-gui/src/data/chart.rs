#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TimeSeriesUnit {
    #[default]
    Time,
    Samples,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmplitudeUnit {
    #[default]
    PercentFullScale,
    DezibelFullScale,
}

impl AmplitudeUnit {
    pub const ALL: [AmplitudeUnit; 2] = [
        AmplitudeUnit::PercentFullScale,
        AmplitudeUnit::DezibelFullScale,
    ];
}

impl std::fmt::Display for AmplitudeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AmplitudeUnit::PercentFullScale => "%FS",
                AmplitudeUnit::DezibelFullScale => "dbFS",
            }
        )
    }
}

impl TimeSeriesUnit {
    pub const ALL: [Self; 2] = [TimeSeriesUnit::Samples, TimeSeriesUnit::Time];
}

impl std::fmt::Display for TimeSeriesUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TimeSeriesUnit::Samples => "Samples",
                TimeSeriesUnit::Time => "Time",
            }
        )
    }
}
