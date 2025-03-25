pub mod file;

use super::{
    measurement::{self},
    ImpulseResponse, Measurement,
};
pub use file::File;

use iced::futures::future::join_all;

use std::path::Path;

#[derive(Debug)]
pub struct Project {
    pub loopback: Option<measurement::Loopback>,
    pub measurements: Vec<Measurement>,
    pub impulse_responses: Vec<ImpulseResponse>,
}

impl Project {
    pub fn new() -> Self {
        Self {
            loopback: None,
            measurements: Vec::new(),
            impulse_responses: Vec::new(),
        }
    }

    pub async fn load(path: impl AsRef<Path>) -> Result<Self, file::Error> {
        let path = path.as_ref();

        let project_file = File::load(path).await?;

        let loopback = match project_file.loopback {
            Some(loopback) => measurement::load_from_file(loopback.path()).await.ok(),
            None => None,
        };

        let measurements = join_all(
            project_file
                .measurements
                .iter()
                .map(|p| measurement::load_from_file(p.path.clone())),
        )
        .await
        .into_iter()
        .flatten()
        .collect();

        Ok(Self {
            loopback,
            measurements,
            impulse_responses: Vec::new(),
        })
    }

    pub fn has_no_measurements(&self) -> bool {
        self.loopback.is_none() && self.measurements.is_empty()
    }
}
