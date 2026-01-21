pub mod config;

pub use config::Config;

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
