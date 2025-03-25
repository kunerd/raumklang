pub mod impulse_responses;
pub mod measurements;

pub use impulse_responses::ImpulseReponses;
pub use measurements::Measurements;

#[derive(Debug, Clone)]
pub enum TabId {
    Measurements,
    ImpulseResponses,
}

pub enum Tab {
    Measurements(Measurements),
    ImpulseResponses(ImpulseReponses),
}

impl Default for Tab {
    fn default() -> Self {
        Self::Measurements(Measurements::new())
    }
}
