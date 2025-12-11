use std::path::PathBuf;

use crate::data;

#[derive(Debug, Clone)]
pub struct Loopback {
    pub name: String,
    pub path: Option<PathBuf>,
    state: State,
}

#[derive(Debug, Clone)]
pub enum State {
    Loaded(raumklang_core::Loopback),
}

impl Loopback {
    pub(crate) fn is_loaded(&self) -> bool {
        matches!(self.state, State::Loaded(_))
    }

    pub(crate) fn new(name: String, inner: raumklang_core::Loopback) -> Self {
        Self {
            name,
            path: None,
            state: State::Loaded(inner),
        }
    }

    pub(crate) fn from_data(loopback: data::Loopback) -> Loopback {
        Self {
            name: loopback.name,
            path: Some(loopback.path),
            state: State::Loaded(loopback.inner),
        }
    }

    pub(crate) fn loaded(&self) -> Option<&raumklang_core::Loopback> {
        match &self.state {
            State::Loaded(loopback) => Some(loopback),
        }
    }
}
