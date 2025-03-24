use std::path::{Path, PathBuf};

use crate::FileError;

use super::Measurement;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectFile {
    pub loopback: Option<ProjectLoopback>,
    pub measurements: Vec<ProjectMeasurement>,
}

impl ProjectFile {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, FileError> {
        let path = path.as_ref();
        let content = tokio::fs::read(path)
            .await
            .map_err(|err| FileError::Io(err.kind()))?;

        let project =
            serde_json::from_slice(&content).map_err(|err| FileError::Json(err.to_string()))?;

        Ok(project)
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectLoopback(ProjectMeasurement);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectMeasurement {
    pub path: PathBuf,
}

impl ProjectLoopback {
    pub fn new(inner: ProjectMeasurement) -> Self {
        Self(inner)
    }

    pub fn path(&self) -> &PathBuf {
        &self.0.path
    }
}

impl<D> From<&Measurement<D>> for ProjectMeasurement {
    fn from(value: &Measurement<D>) -> Self {
        Self {
            path: value.path.to_path_buf(),
        }
    }
}
