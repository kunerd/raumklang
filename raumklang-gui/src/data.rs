pub mod impulse_response;
pub mod measurement;
pub mod project;
mod recent_projects;

pub use impulse_response::ImpulseResponse;
pub use measurement::Measurement;
pub use project::Project;
pub use recent_projects::RecentProjects;

use std::{io, sync::Arc};

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("io operation failed: {0}")]
    IOFailed(Arc<io::Error>),
    #[error("deserialization failed: {0}")]
    SerdeFailed(Arc<serde_json::Error>),
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
