use super::FromFile;
use crate::data::SampleRate;

use raumklang_core::WavLoadError;

use std::path::{Path, PathBuf};

#[derive(Debug, Clone)]
pub struct Loopback {
    pub path: PathBuf,
    pub state: State,
}

#[derive(Debug, Default, Clone)]
pub enum State {
    #[default]
    NotLoaded,
    Loaded(raumklang_core::Loopback),
}

impl Loopback {
    pub fn sample_rate(&self) -> Option<SampleRate> {
        if let State::Loaded(signal) = &self.state {
            Some(SampleRate::new(signal.sample_rate()))
        } else {
            None
        }
    }
}

impl FromFile for Loopback {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized,
    {
        let path = path.as_ref();

        let state = match raumklang_core::Loopback::from_file(path) {
            Ok(data) => State::Loaded(data),
            Err(_) => State::NotLoaded,
        };

        let path = path.to_path_buf();
        Ok(Self { path, state })
    }
}
