use std::{
    path::PathBuf,
    sync::atomic::{self, AtomicUsize},
};

use crate::data;

#[derive(Debug, Clone)]
pub struct Measurement {
    pub id: Id,
    pub name: String,
    pub path: Option<PathBuf>,
    pub inner: State<raumklang_core::Measurement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(pub usize);

#[derive(Debug, Clone)]
pub enum State<T> {
    NotLoaded,
    Loaded(T),
}

impl<T> State<T> {
    pub(crate) fn loaded(&self) -> Option<&T> {
        match self {
            State::NotLoaded => None,
            State::Loaded(ref inner) => Some(inner),
        }
    }

    pub fn is_loaded(&self) -> bool {
        matches!(self, State::Loaded(_))
    }
}

impl Measurement {
    pub fn new(name: String, measurement: raumklang_core::Measurement) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self {
            id,
            name,
            path: None,
            inner: State::Loaded(measurement),
        }
    }

    pub fn from_data(measurement: data::Measurement) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self {
            id,
            name: measurement.name,
            path: Some(measurement.path),
            inner: State::Loaded(measurement.inner),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }
}

#[derive(Debug, Clone)]
pub struct Loopback {
    pub name: String,
    pub path: Option<PathBuf>,
    pub inner: State<raumklang_core::Loopback>,
}

impl Loopback {
    pub fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }

    pub(crate) fn new(name: String, inner: raumklang_core::Loopback) -> Self {
        Self {
            name,
            path: None,
            inner: State::Loaded(inner),
        }
    }

    pub(crate) fn from_data(loopback: data::Loopback) -> Loopback {
        Self {
            name: loopback.name,
            path: Some(loopback.path),
            inner: State::Loaded(loopback.inner),
        }
    }

    pub(crate) fn loaded(&self) -> Option<&raumklang_core::Loopback> {
        self.inner.loaded()
    }
}
