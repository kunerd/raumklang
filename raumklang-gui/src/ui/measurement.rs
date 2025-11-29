pub mod loopback;

pub use loopback::Loopback;

use std::{
    fmt::Display,
    path::PathBuf,
    sync::atomic::{self, AtomicUsize},
};

use crate::{data, ui};

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
    analysis: Analysis,
}

#[derive(Debug, Clone)]
pub enum Analysis {
    None,
    ComputingImpulseResponse,
    ImpulseResponseComputed {
        result: ui::ImpulseResponse,
        frequency_response: Option<()>,
    },
}

impl State {
    pub(crate) fn loaded(&self) -> Option<&Loaded> {
        match self {
            State::NotLoaded(_) => None,
            State::Loaded(l) => Some(l),
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, State::Loaded { .. })
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
