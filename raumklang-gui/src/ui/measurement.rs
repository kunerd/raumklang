pub mod loopback;

pub use loopback::Loopback;

use std::{
    fmt::Display,
    path::PathBuf,
    sync::atomic::{self, AtomicUsize},
};

use crate::{
    data,
    ui::{self, frequency_response},
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
    NotLoaded(NotLoaded),
    Loaded(Loaded),
}

impl State {
    pub(crate) fn name(&self) -> &String {
        match self {
            State::NotLoaded(not_loaded) => &not_loaded.name,
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
            State::NotLoaded(_) => None,
            State::Loaded(l) => Some(l),
        }
    }

    pub(crate) fn loaded_mut(&mut self) -> Option<&mut Loaded> {
        match self {
            State::NotLoaded(_) => None,
            State::Loaded(l) => Some(l),
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, State::Loaded { .. })
    }
}

#[derive(Debug, Clone)]
pub struct NotLoaded {
    id: Id,
    name: String,
    path: Option<PathBuf>,
}

#[derive(Debug, Clone)]
pub struct Loaded {
    pub id: Id,
    pub name: String,
    pub path: Option<PathBuf>,
    pub data: raumklang_core::Measurement,
    pub analysis: Analysis,
}

#[derive(Debug, Clone)]
pub enum Analysis {
    None,
    ComputingImpulseResponse,
    ImpulseResponseComputed {
        result: ui::ImpulseResponse,
        frequency_response: ui::FrequencyResponse,
    },
}

impl Analysis {
    pub fn impulse_response(&self) -> Option<&ui::ImpulseResponse> {
        let Analysis::ImpulseResponseComputed { result, .. } = self else {
            return None;
        };

        Some(result)
    }

    pub(crate) fn set_impulse_response(&mut self, impulse_response: ui::ImpulseResponse) {
        *self = Analysis::ImpulseResponseComputed {
            result: impulse_response,
            frequency_response: ui::FrequencyResponse::default(),
        }
    }

    pub fn frequency_response(&self) -> Option<&ui::FrequencyResponse> {
        let Analysis::ImpulseResponseComputed {
            frequency_response, ..
        } = self
        else {
            return None;
        };

        Some(frequency_response)
    }

    pub(crate) fn frequency_response_mut(&mut self) -> Option<&mut ui::FrequencyResponse> {
        let Analysis::ImpulseResponseComputed {
            ref mut frequency_response,
            ..
        } = self
        else {
            return None;
        };

        Some(frequency_response)
    }

    pub(crate) fn set_frequency_response(&mut self, data: data::FrequencyResponse) {
        let Analysis::ImpulseResponseComputed {
            ref mut frequency_response,
            ..
        } = self
        else {
            return;
        };

        frequency_response.state = frequency_response::State::Computed(data);
    }

    pub(crate) fn reset_frequency_response(&mut self) {
        let Analysis::ImpulseResponseComputed {
            ref mut frequency_response,
            ..
        } = self
        else {
            return;
        };

        frequency_response.state = frequency_response::State::ComputingFrequencyResponse;
    }

    pub(crate) fn apply(&mut self, _event: data::impulse_response::Event) {
        *self = Analysis::ComputingImpulseResponse;
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
            analysis: Analysis::None,
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
            analysis: Analysis::None,
        }
    }
}
