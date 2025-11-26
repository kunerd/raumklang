use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub loopback: Option<Loopback>,
    pub measurements: Vec<Measurement>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Loopback(pub Measurement);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Measurement {
    pub path: PathBuf,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    #[error("could not load file: {0}")]
    Io(io::ErrorKind),
    #[error("could not parse file: {0}")]
    Json(String),
}

impl Project {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let content = tokio::fs::read(path)
            .await
            .map_err(|err| Error::Io(err.kind()))?;

        let project =
            serde_json::from_slice(&content).map_err(|err| Error::Json(err.to_string()))?;

        Ok(project)
    }

    pub async fn save(self, path: impl AsRef<Path>) -> Result<(), Error> {
        let json =
            serde_json::to_string_pretty(&self).map_err(|err| Error::Json(err.to_string()))?;

        tokio::fs::write(path, json)
            .await
            .map_err(|err| Error::Io(err.kind()))?;

        Ok(())
    }
}
