mod audio;
mod impulse_response;
mod window;

pub mod loudness;
pub mod signals;

pub use audio::*;
pub use impulse_response::*;
pub use window::*;

use signals::map_hound_error;

use thiserror::Error;

use std::{
    io::{self},
    path::Path,
    slice::Iter,
    time::SystemTime,
};

#[derive(Debug, Clone)]
pub struct Loopback(Measurement);

#[derive(Debug, Clone)]
pub struct Measurement {
    sample_rate: u32,
    data: Vec<f32>,
    pub modified: SystemTime,
}

impl Loopback {
    pub fn new(inner: Measurement) -> Self {
        Self(inner)
    }

    pub fn iter(&self) -> Iter<'_, f32> {
        self.0.iter()
    }

    pub fn sample_rate(&self) -> u32 {
        self.0.sample_rate()
    }

    pub fn duration(&self) -> usize {
        self.0.duration()
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let measurement = Measurement::from_file(path)?;

        Ok(Self(measurement))
    }
}

impl AsRef<Measurement> for Loopback {
    fn as_ref(&self) -> &Measurement {
        &self.0
    }
}

impl Measurement {
    pub fn new(sample_rate: u32, data: Vec<f32>) -> Self {
        Self {
            sample_rate,
            data,
            modified: SystemTime::now(),
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let file = std::fs::File::open(path)?;
        // let mut file = hound::WavReader::open(file).map_err(map_hound_error)?;
        let modified = file.metadata()?.modified()?;
        let mut file = hound::WavReader::new(file).map_err(map_hound_error)?;

        let sample_rate = file.spec().sample_rate;
        let data: Vec<f32> = file
            .samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(map_hound_error)?;

        Ok(Measurement {
            sample_rate,
            data,
            modified,
        })
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn duration(&self) -> usize {
        self.data.len()
    }

    pub fn iter(&self) -> Iter<'_, f32> {
        self.data.iter()
    }
}

impl From<Loopback> for Measurement {
    fn from(loopback: Loopback) -> Self {
        loopback.0
    }
}

pub fn volume_to_amplitude(volume: f32) -> f32 {
    assert!((0.0..=1.0).contains(&volume));

    // FIXME:
    // 1. remove magic numbers
    // https://www.dr-lex.be/info-stuff/volumecontrols.html
    let a = 0.001;
    let b = 6.908;

    if volume < 0.1 {
        volume * 10.0 * a * f32::exp(0.1 * b)
    } else {
        a * f32::exp(b * volume)
    }
}

#[inline]
pub fn dbfs(v: f32) -> f32 {
    20.0 * f32::log10(v.abs())
}

#[derive(Error, Debug)]
pub enum Error {
    #[error("error laoding a measurement")]
    WavLoadFile(#[from] WavLoadError),
    #[error(transparent)]
    AudioBackend(#[from] AudioBackendError),
}

#[derive(Error, Debug)]
pub enum WavLoadError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("unknown")]
    Other,
}
