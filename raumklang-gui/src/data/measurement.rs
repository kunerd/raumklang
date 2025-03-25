use std::path::{Path, PathBuf};

use raumklang_core::WavLoadError;

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

pub trait FromFile {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized;
}

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
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let state = match raumklang_core::Loopback::from_file(path) {
            Ok(data) => State::Loaded(data),
            Err(_) => State::NotLoaded,
        };

        let path = path.to_path_buf();
        Ok(Measurement { name, path, state })
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
            Ok(data) => State::Loaded(data),
            Err(_) => State::NotLoaded,
        };

        Ok(Self {
            name,
            path: path.to_path_buf(),
            state,
        })
    }
}
