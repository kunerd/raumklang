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
pub enum State {
    NotLoaded(NotLoaded),
    Loaded(Loaded),
}

#[derive(Debug, Clone)]
pub struct NotLoaded {
    id: Id,
    name: String,
    path: Option<PathBuf>,
}

impl State {
    pub(crate) fn name(&self) -> &String {
        match self {
            State::Loaded(inner) => &inner.name,
            State::NotLoaded(inner) => &inner.name,
        }
    }

    pub(crate) fn new(name: String, data: raumklang_core::Measurement) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self::Loaded(Loaded {
            id,
            name,
            path: None,
            data,
            analysis: Analysis::default(),
        })
    }

    pub async fn from_file(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        static ID: AtomicUsize = AtomicUsize::new(0);
        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        match raumklang_core::Measurement::from_file(path) {
            Ok(inner) => {
                let loaded = Loaded {
                    id,
                    name,
                    path: Some(path.to_path_buf()),
                    data: inner,
                    analysis: Analysis::default(),
                };

                State::Loaded(loaded)
            }
            Err(_err) => {
                let inner = NotLoaded {
                    id,
                    name,
                    path: Some(path.to_path_buf()),
                };

                State::NotLoaded(inner)
            }
        }
    }

    pub(crate) fn loaded(&self) -> Option<&Loaded> {
        match self {
            State::Loaded(l) => Some(l),
            State::NotLoaded(_) => None,
        }
    }

    pub(crate) fn loaded_mut(&mut self) -> Option<&mut Loaded> {
        match self {
            State::Loaded(l) => Some(l),
            State::NotLoaded(_) => None,
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, State::Loaded { .. })
    }

    pub(crate) fn id(&self) -> Id {
        match self {
            State::NotLoaded(not_loaded) => not_loaded.id,
            State::Loaded(loaded) => loaded.id,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub id: Id,
    pub name: String,
    pub path: Option<PathBuf>,
    pub data: raumklang_core::Measurement,
    pub analysis: Analysis,
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
