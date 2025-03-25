pub mod measurements;
pub use measurements::Measurements;

#[derive(Debug, Clone)]
pub enum TabId {
    Measurements,
    ImpulseResponses,
}

pub enum Tab {
    Measurements(Measurements),
    ImpulseResponses,
}

impl Default for Tab {
    fn default() -> Self {
        Self::Measurements(Measurements::new())
    }
}
