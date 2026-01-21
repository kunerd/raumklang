pub mod loopback;

pub use loopback::Loopback;

use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::atomic::{self, AtomicUsize},
};

use crate::ui::{impulse_response, spectral_decay, spectrogram, FrequencyResponse};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(usize);

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub struct Measurement {
    id: Id,
    pub name: String,
    pub path: Option<PathBuf>,
    pub state: State,
}

#[derive(Debug, Clone)]
pub enum State {
    NotLoaded,
    Loaded {
        signal: raumklang_core::Measurement,
        analysis: Analysis,
    },
}

impl Measurement {
    pub(crate) fn new(name: String, path: Option<PathBuf>, state: State) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self {
            id,
            name,
            path,
            state,
        }
    }

    pub async fn from_file(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let state = match raumklang_core::Measurement::from_file(path) {
            Ok(signal) => State::Loaded {
                signal,
                analysis: Analysis::default(),
            },
            Err(_err) => State::NotLoaded,
        };

        let path = Some(path.to_path_buf());
        Self::new(name, path, state)
    }

    pub fn is_loaded(&self) -> bool {
        match &self.state {
            State::NotLoaded => false,
            State::Loaded { .. } => true,
        }
    }

    pub fn signal(&self) -> Option<&raumklang_core::Measurement> {
        match &self.state {
            State::NotLoaded => None,
            State::Loaded { signal, .. } => Some(signal),
        }
    }

    pub(crate) fn id(&self) -> Id {
        self.id
    }
}

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub impulse_response: impulse_response::State,
    pub frequency_response: FrequencyResponse,
    pub spectral_decay: spectral_decay::State,
    pub spectrogram: spectrogram::State,
}

impl Analysis {
    pub(crate) fn spectral_decay_progress(&self) -> spectral_decay::Progress {
        match self.impulse_response {
            impulse_response::State::None => spectral_decay::Progress::None,
            impulse_response::State::Computing => {
                spectral_decay::Progress::ComputingImpulseResponse
            }
            impulse_response::State::Computed(_) => match self.spectral_decay {
                spectral_decay::State::None => spectral_decay::Progress::None,
                spectral_decay::State::Computing => spectral_decay::Progress::Computing,
                spectral_decay::State::Computed(_) => spectral_decay::Progress::Finished,
            },
        }
    }

    pub(crate) fn spectrogram_progress(&self) -> spectrogram::Progress {
        match self.impulse_response {
            impulse_response::State::None => spectrogram::Progress::None,
            impulse_response::State::Computing => spectrogram::Progress::ComputingImpulseResponse,
            impulse_response::State::Computed(_) => match self.spectrogram {
                spectrogram::State::None => spectrogram::Progress::None,
                spectrogram::State::Computing => spectrogram::Progress::Computing,
                spectrogram::State::Computed(_) => spectrogram::Progress::Finished,
            },
        }
    }
}
