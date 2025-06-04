use super::{Details, FromFile, State};
use crate::data::SampleRate;

use raumklang_core::WavLoadError;

use std::path::Path;

#[derive(Debug, Clone)]
pub struct Loopback {
    inner: raumklang_core::Loopback,
    details: Details,
}

impl Loopback {
    pub fn new(inner: raumklang_core::Loopback, details: Details) -> Self {
        Self { inner, details }
    }
}

impl AsRef<raumklang_core::Loopback> for Loopback {
    fn as_ref(&self) -> &raumklang_core::Loopback {
        &self.inner
    }
}

impl State<Loopback> {
    pub fn sample_rate(&self) -> Option<SampleRate> {
        if let State::Loaded(signal) = &self {
            Some(SampleRate::new(signal.inner.sample_rate()))
        } else {
            None
        }
    }
}

impl FromFile for State<Loopback> {
    fn from_file(path: impl AsRef<Path>) -> Result<State<Loopback>, WavLoadError>
    where
        Self: Sized,
    {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let details = Details {
            name,
            path: path.to_path_buf(),
        };

        Ok(match raumklang_core::Loopback::from_file(path) {
            Ok(data) => State::Loaded(Loopback {
                inner: data,
                details,
            }),
            Err(_) => State::NotLoaded(details),
        })
    }
}
