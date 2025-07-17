use std::{
    path::{Path, PathBuf},
    sync::atomic::{self, AtomicUsize},
};

use crate::data;

#[derive(Debug)]
pub struct Measurement {
    pub id: Id,
    pub name: String,
    pub path: PathBuf,
    pub inner: State<raumklang_core::Measurement>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(usize);

#[derive(Debug)]
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
    pub fn from_data(measurement: data::Measurement) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self {
            id,
            name: measurement.name,
            path: measurement.path,
            inner: State::Loaded(measurement.inner),
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }
}

#[derive(Debug)]
pub struct Loopback {
    pub name: String,
    pub path: PathBuf,
    pub inner: State<raumklang_core::Loopback>,
}

impl Loopback {
    pub fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }

    pub(crate) fn from_data(loopback: data::Loopback) -> Loopback {
        Self {
            name: loopback.name,
            path: loopback.path,
            inner: State::Loaded(loopback.inner),
        }
    }
}
