use std::time::Duration;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    pub duration: Duration,
    pub start_frequency: u16,
    pub end_frequency: u16,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            duration: Duration::from_secs(5),
            // human hearing range
            start_frequency: 20,
            end_frequency: 20_000,
        }
    }
}
