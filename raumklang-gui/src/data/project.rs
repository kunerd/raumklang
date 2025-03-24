use iced::futures::future::join_all;
use raumklang_core::WavLoadError;

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Project {
    pub loopback: Option<Loopback>,
    pub measurements: Vec<Measurement>,
}

pub type Loopback = Measurement<raumklang_core::Loopback>;

#[derive(Debug)]
pub struct Measurement<D = raumklang_core::Measurement> {
    pub name: String,
    pub path: PathBuf,
    pub state: State<D>,
}

#[derive(Debug, Default)]
pub enum State<D> {
    #[default]
    NotLoaded,
    Loaded(D),
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

impl FromFile for Loopback {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized,
    {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let data = raumklang_core::Loopback::from_file(path)?;

        let path = path.to_path_buf();
        Ok(Measurement {
            name,
            path,
            state: State::Loaded(data),
        })
    }
}

pub trait FromFile {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized;
}

impl FromFile for Measurement {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let data = raumklang_core::Measurement::from_file(path)?;

        Ok(Self {
            name,
            path: path.to_path_buf(),
            state: State::Loaded(data),
        })
    }
}
