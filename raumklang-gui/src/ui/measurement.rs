pub mod loopback;

pub use loopback::Loopback;

use std::{
    fmt::Display,
    path::PathBuf,
    sync::atomic::{self, AtomicUsize},
};

use crate::{
    data,
    ui::{impulse_response, spectral_decay, spectrogram, FrequencyResponse},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(usize);

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}

#[derive(Debug, Clone)]
pub enum State {
    Loaded(Loaded),
}

impl State {
    pub(crate) fn name(&self) -> &String {
        match self {
            State::Loaded(loaded) => &loaded.name,
        }
    }

    pub(crate) fn new(name: String, data: raumklang_core::Measurement) -> Self {
        Self::Loaded(Loaded::new(name, data))
    }

    pub(crate) fn from_data(data: data::Measurement) -> Self {
        Self::Loaded(Loaded::from_data(data))
    }

    pub(crate) fn loaded(&self) -> Option<&Loaded> {
        match self {
            State::Loaded(l) => Some(l),
        }
    }

    pub(crate) fn loaded_mut(&mut self) -> Option<&mut Loaded> {
        match self {
            State::Loaded(l) => Some(l),
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, State::Loaded { .. })
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

impl Loaded {
    pub fn new(name: String, data: raumklang_core::Measurement) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self {
            id,
            name,
            path: None,
            data,
            analysis: Analysis::default(),
        }
    }

    pub fn from_data(measurement: data::Measurement) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self {
            id,
            name: measurement.name,
            path: Some(measurement.path),
            data: measurement.inner,
            analysis: Analysis::default(),
        }
    }
}
