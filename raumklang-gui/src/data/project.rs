use super::FromFile;

use iced::futures::future::join_all;
use raumklang_core::WavLoadError;

use std::path::Path;

#[derive(Debug)]
pub struct Project {
    pub loopback: Option<super::Loopback>,
    pub measurements: Vec<super::Measurement>,
}

impl Project {
    pub fn new() -> Self {
        Self {
            loopback: None,
            measurements: Vec::new(),
        }
    }

    pub async fn load(project_file: super::ProjectFile) -> Self {
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

        Self {
            loopback,
            measurements,
        }
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
