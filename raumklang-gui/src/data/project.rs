pub mod file;

use super::{
    measurement::{self, FromFile},
    Measurement,
};
pub use file::File;

use raumklang_core::WavLoadError;

use iced::futures::future::join_all;

use std::path::Path;

#[derive(Debug)]
pub struct Project {
    pub loopback: Option<measurement::Loopback>,
    pub measurements: Vec<Measurement>,
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
            Some(loopback) => Self::load_signal_from_file(loopback.path()).await.ok(),
            None => None,
        };

        let measurements = join_all(
            project_file
                .measurements
                .iter()
                .map(|p| Self::load_signal_from_file(p.path.clone())),
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

    pub async fn load_signal_from_file<P, T>(path: P) -> Result<T, WavLoadError>
    where
        T: FromFile + Send + 'static,
        P: AsRef<Path> + Send + Sync,
    {
        let path = path.as_ref().to_owned();
        tokio::task::spawn_blocking(move || T::from_file(path))
            .await
            .map_err(|_err| WavLoadError::Other)?
    }

    pub fn has_no_measurements(&self) -> bool {
        self.loopback.is_none() && self.measurements.is_empty()
    }
}
