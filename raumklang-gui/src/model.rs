use std::path::{Path, PathBuf};

use crate::tabs::measurements::WavLoadError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub loopback: Option<ProjectLoopback>,
    pub measurements: Vec<ProjectMeasurement>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectLoopback(ProjectMeasurement);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectMeasurement {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Measurement {
    pub name: String,
    pub path: PathBuf,
    pub sample_rate: u32,
    pub data: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct Loopback(Measurement);

impl ProjectLoopback {
    pub fn path(&self) -> &PathBuf {
        &self.0.path
    }
}

impl Measurement {
    pub fn new(name: String, sample_rate: u32, data: Vec<f32>) -> Self {
        Self {
            name,
            path: PathBuf::new(),
            sample_rate,
            data,
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let mut loopback =
            hound::WavReader::open(path).map_err(|err| map_hound_error(path, err))?;
        let sample_rate = loopback.spec().sample_rate;
        // only mono files
        // currently only 32bit float
        let data = loopback
            .samples::<f32>()
            .collect::<hound::Result<Vec<f32>>>()
            .map_err(|err| map_hound_error(path, err))?;

        Ok(Self {
            name,
            path: path.to_path_buf(),
            sample_rate,
            data,
        })
    }
}

fn map_hound_error(path: impl AsRef<Path>, err: hound::Error) -> WavLoadError {
    let path = path.as_ref().to_path_buf();
    match err {
        hound::Error::IoError(err) => WavLoadError::IoError(path, err.kind()),
        _ => WavLoadError::Other,
    }
}
