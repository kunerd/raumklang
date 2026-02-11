use serde::{Deserialize, Serialize};
use tokio::fs;

use std::{
    fmt, io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub loopback: Option<Loopback>,
    pub measurements: Vec<Measurement>,
    #[serde(default)]
    pub measurement_operation: Operation,
    #[serde(default)]
    pub export_from_memory: bool,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Loopback(pub Measurement);

impl Loopback {
    pub fn new(path: PathBuf) -> Self {
        Self(Measurement { path })
    }

    pub async fn copy(&mut self, dest: impl AsRef<Path>) {
        self.0.copy(dest).await
    }

    pub async fn rename(&mut self, dest: impl AsRef<Path>) {
        self.0.rename(dest).await
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Measurement {
    pub path: PathBuf,
}

impl Measurement {
    pub fn new(path: PathBuf) -> Self {
        Self { path }
    }

    pub async fn copy(&mut self, dest: impl AsRef<Path>) {
        let dest = dest.as_ref().with_file_name(self.path.file_name().unwrap());

        if self.path == dest {
            return;
        }

        dbg!(&self.path, &dest);
        fs::copy(&self.path, &dest).await.unwrap();

        self.path = dest;
    }

    // WARNING: will only work when both files are on the same mount point
    pub async fn rename(&mut self, dest: impl AsRef<Path>) {
        let dest = dest.as_ref().with_file_name(self.path.file_name().unwrap());

        if self.path == dest {
            return;
        }

        fs::rename(&self.path, &dest).await.unwrap();

        self.path = dest;
    }
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

    pub async fn save(mut self, path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();

        if let Some(parent) = path.parent() {
            fs::create_dir_all(&parent).await.unwrap();
        }

        match self.measurement_operation {
            Operation::None => {}
            Operation::Copy => {
                if let Some(loopback) = self.loopback.as_mut() {
                    loopback.copy(&path).await;
                }

                for m in self.measurements.iter_mut() {
                    m.copy(&path).await;
                }
            }
            Operation::Move => {
                if let Some(loopback) = self.loopback.as_mut() {
                    loopback.rename(&path).await;
                }

                for m in self.measurements.iter_mut() {
                    m.rename(&path).await;
                }
            }
        }

        let json =
            serde_json::to_string_pretty(&self).map_err(|err| Error::Json(err.to_string()))?;

        tokio::fs::write(path, json)
            .await
            .map_err(|err| Error::Io(err.kind()))?;

        Ok(self)
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Operation {
    #[default]
    None,
    Copy,
    Move,
}

impl Operation {
    pub const ALL: &[Operation] = &[Operation::None, Operation::Copy, Operation::Move];
}

impl fmt::Display for Operation {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let s = match self {
            Operation::None => "do nothing",
            Operation::Copy => "copy to project directory",
            Operation::Move => "move to project directory",
        };

        write!(f, "{}", s)
    }
}
