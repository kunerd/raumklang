pub mod impulse_response;
pub mod measurement;
pub mod project;
mod recent_projects;

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

pub async fn compute_impulse_response(
    id: usize,
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
) -> Result<(usize, raumklang_core::ImpulseResponse), Error> {
    let impulse_response = tokio::task::spawn_blocking(move || {
        raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement).unwrap()
    })
    .await
    .unwrap();

    Ok((id, impulse_response))
}
