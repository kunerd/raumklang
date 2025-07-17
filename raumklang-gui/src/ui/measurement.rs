use std::{
    path::{Path, PathBuf},
    sync::atomic::{self, AtomicUsize},
};

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
    fn from_state(name: String, path: PathBuf, inner: State<raumklang_core::Measurement>) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        Self {
            id,
            name,
            path,
            inner,
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }

    pub async fn from_file(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let inner = match raumklang_core::Measurement::from_file(path) {
            Ok(inner) => State::Loaded(inner),
            Err(_) => State::NotLoaded,
        };

        Self::from_state(name, path.to_path_buf(), inner)
    }
}

#[derive(Debug)]
pub struct Loopback {
    pub name: String,
    pub path: PathBuf,
    pub inner: State<raumklang_core::Loopback>,
}

impl Loopback {
    fn from_measurement(measurement: Measurement) -> Self {
        Self {
            name: measurement.name,
            path: measurement.path,
            inner: match measurement.inner {
                State::NotLoaded => State::NotLoaded,
                State::Loaded(inner) => State::Loaded(raumklang_core::Loopback::new(inner)),
            },
        }
    }

    pub fn is_loaded(&self) -> bool {
        self.inner.is_loaded()
    }

    pub async fn from_file(path: impl AsRef<Path>) -> Self {
        let measurement = Measurement::from_file(path).await;

        Self::from_measurement(measurement)
    }
}
