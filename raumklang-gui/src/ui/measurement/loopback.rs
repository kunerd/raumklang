use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct Loopback {
    pub name: String,
    pub path: Option<PathBuf>,
    pub state: State,
}

#[derive(Debug, Clone)]
pub enum State {
    Loaded(raumklang_core::Loopback),
    NotLoaded(Arc<raumklang_core::WavLoadError>),
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

    pub async fn from_file(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let state = match raumklang_core::Loopback::from_file(path) {
            Ok(inner) => State::Loaded(inner),
            Err(err) => State::NotLoaded(Arc::new(err)),
        };

        Self {
            name,
            path: Some(path.to_path_buf()),
            state,
        }
    }

    pub(crate) fn loaded(&self) -> Option<&raumklang_core::Loopback> {
        match &self.state {
            State::Loaded(loopback) => Some(loopback),
            State::NotLoaded(_) => None,
        }
    }
}
