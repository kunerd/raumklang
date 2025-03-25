pub mod file;

use super::{
    impulse_response,
    measurement::{self},
    Measurement,
};
pub use file::File;

use iced::futures::future::join_all;

use std::path::Path;

#[derive(Debug)]
pub struct Project {
    loopback: Option<measurement::Loopback>,
    measurements: Vec<Measurement>,
}

impl Project {
    pub fn new() -> Self {
        Self {
            loopback: None,
            measurements: Vec::new(),
        }
    }

    pub async fn load(path: impl AsRef<Path>) -> Result<Self, file::Error> {
        let path = path.as_ref();
        let project_file = File::load(path).await?;

        let loopback = match project_file.loopback {
            Some(loopback) => measurement::load_from_file(loopback.path()).await.ok(),
            None => None,
        };

        let measurements: Vec<_> = join_all(
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
        })
    }

    pub fn loopback(&self) -> Option<&measurement::Loopback> {
        self.loopback.as_ref()
    }

    pub fn measurements(&self) -> &[Measurement] {
        &self.measurements
    }

    pub fn measurements_mut(&mut self) -> &mut [Measurement] {
        &mut self.measurements
    }

    pub fn has_no_measurements(&self) -> bool {
        self.loopback.is_none() && self.measurements.is_empty()
    }

    pub fn set_loopback(&mut self, loopback: Option<measurement::Loopback>) {
        self.loopback = loopback;

        self.measurements
            .iter_mut()
            .for_each(|m| match &mut m.state {
                measurement::State::NotLoaded => {}
                measurement::State::Loaded {
                    impulse_response, ..
                } => *impulse_response = impulse_response::State::NotComputed,
            });
    }

    pub fn push_measurements(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }

    pub fn remove_measurement(&mut self, index: usize) -> Measurement {
        self.measurements.remove(index)
    }
}
