pub mod chart;
pub mod frequency_response;
pub mod impulse_response;
pub mod measurement;
pub mod project;
mod recent_projects;
pub mod recording;
mod sample_rate;
mod samples;
pub mod window;

pub use frequency_response::FrequencyResponse;
pub use impulse_response::ImpulseResponse;
pub use measurement::Measurement;
pub use project::Project;
pub use recent_projects::RecentProjects;
pub use sample_rate::SampleRate;
pub use samples::Samples;
pub use window::Window;

use std::{io, sync::Arc};

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("io operation failed: {0}")]
    IOFailed(Arc<io::Error>),
    #[error("deserialization failed: {0}")]
    SerdeFailed(Arc<serde_json::Error>),
    #[error("impulse response computation failed")]
    ImpulseResponseComputationFailed,
}

impl From<io::Error> for Error {
    fn from(error: io::Error) -> Self {
        Self::IOFailed(Arc::new(error))
    }
}

impl From<serde_json::Error> for Error {
    fn from(error: serde_json::Error) -> Self {
        Self::SerdeFailed(Arc::new(error))
    }
}
