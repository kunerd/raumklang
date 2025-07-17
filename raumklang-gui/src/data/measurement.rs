mod config;

pub use config::Config;

use raumklang_core::WavLoadError;

use std::path::{Path, PathBuf};

#[derive(Debug)]
pub struct Measurement {
    pub name: String,
    pub path: PathBuf,
    pub inner: raumklang_core::Measurement,
}

impl Measurement {
    pub async fn from_file(path: impl AsRef<Path>) -> Result<Self, raumklang_core::WavLoadError> {
        let path = path.as_ref();

        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let inner = raumklang_core::Measurement::from_file(path)?;

        Ok(Self {
            name,
            path: path.to_path_buf(),
            inner,
        })
    }
}

#[derive(Debug)]
pub struct Loopback {
    pub name: String,
    pub path: PathBuf,
    pub inner: raumklang_core::Loopback,
}

impl Loopback {
    fn from_measurement(measurement: Measurement) -> Self {
        Self {
            name: measurement.name,
            path: measurement.path,
            inner: raumklang_core::Loopback::new(measurement.inner),
        }
    }

    pub async fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let measurement = Measurement::from_file(path).await?;

        Ok(Self::from_measurement(measurement))
    }
}
