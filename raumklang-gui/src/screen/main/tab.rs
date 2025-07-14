pub mod frequency_responses;
pub mod impulse_responses;
pub mod measurements;

pub use frequency_responses::FrequencyResponses;
pub use impulse_responses::ImpulseReponses;
pub use measurements::Measurements;

pub enum Tab {
    Measurements(Measurements),
    ImpulseResponses,
    FrequencyResponses,
}

impl Default for Tab {
    fn default() -> Self {
        Self::Measurements(Measurements::new())
    }
}
