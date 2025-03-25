use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct File {
    pub loopback: Option<Loopback>,
    pub measurements: Vec<Measurement>,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    #[error("could not load file: {0}")]
    Io(io::ErrorKind),
    #[error("could not parse file: {0}")]
    Json(String),
}

impl File {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let content = tokio::fs::read(path)
            .await
            .map_err(|err| Error::Io(err.kind()))?;

        let project =
            serde_json::from_slice(&content).map_err(|err| Error::Json(err.to_string()))?;

        Ok(project)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Loopback(Measurement);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Measurement {
    pub path: PathBuf,
}

impl Loopback {
    pub fn new(inner: Measurement) -> Self {
        Self(inner)
    }

    pub fn path(&self) -> &PathBuf {
        &self.0.path
    }
}

impl<D> From<&super::Measurement<D>> for Measurement {
    fn from(value: &super::Measurement<D>) -> Self {
        Self {
            path: value.path.to_path_buf(),
        }
    }
}
