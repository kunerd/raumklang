use std::path::{Path, PathBuf};

use raumklang_core::WavLoadError;

use super::impulse_response;

// pub type Loopback = Measurement<raumklang_core::Loopback>;

#[derive(Debug, Clone)]
pub struct Loopback {
    pub path: PathBuf,
    pub state: LoopbackState,
}

#[derive(Debug, Default, Clone)]
pub enum LoopbackState {
    #[default]
    NotLoaded,
    Loaded(raumklang_core::Loopback),
}

#[derive(Debug, Clone)]
pub struct Measurement {
    pub name: String,
    pub path: PathBuf,
    pub state: MeasurementState,
}

#[derive(Debug, Default, Clone)]
pub enum MeasurementState {
    #[default]
    NotLoaded,
    Loaded {
        data: raumklang_core::Measurement,
        impulse_response: impulse_response::State,
    },
}

pub trait FromFile {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized;
}

// impl Measurement<raumklang_core::Measurement> {
//     pub async fn compute_impulse_response()

// }

pub async fn load_from_file<P, T>(path: P) -> Result<T, WavLoadError>
where
    T: FromFile + Send + 'static,
    P: AsRef<Path> + Send + Sync,
{
    let path = path.as_ref().to_owned();
    tokio::task::spawn_blocking(move || T::from_file(path))
        .await
        .map_err(|_err| WavLoadError::Other)?
}

impl FromFile for Loopback {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized,
    {
        let path = path.as_ref();

        let state = match raumklang_core::Loopback::from_file(path) {
            Ok(data) => LoopbackState::Loaded(data),
            Err(_) => LoopbackState::NotLoaded,
        };

        let path = path.to_path_buf();
        Ok(Self { path, state })
    }
}

impl FromFile for Measurement {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let state = match raumklang_core::Measurement::from_file(path) {
            Ok(data) => MeasurementState::Loaded {
                data,
                impulse_response: impulse_response::State::NotComputed,
            },
            Err(_) => MeasurementState::NotLoaded,
        };

        Ok(Self {
            name,
            path: path.to_path_buf(),
            state,
        })
    }
}
