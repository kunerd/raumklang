pub mod config;
pub mod loopback;

pub use config::Config;
pub use loopback::Loopback;

use raumklang_core::WavLoadError;

use std::{
    path::{Path, PathBuf},
    slice,
    sync::atomic::{self, AtomicUsize},
};

#[derive(Debug, Default)]
pub struct List(Vec<State<Measurement>>);

impl List {
    pub(crate) fn from_iter(iter: impl IntoIterator<Item = State<Measurement>>) -> Self {
        Self(iter.into_iter().collect())
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = &State<Measurement>> {
        self.0.iter()
    }

    pub(crate) fn into_loaded(self) -> impl Iterator<Item = Measurement> {
        self.0.into_iter().filter_map(|s| match s {
            State::NotLoaded(_details) => None,
            State::Loaded(measurement) => Some(measurement),
        })
    }

    pub(crate) fn into_iter(self) -> impl Iterator<Item = State<Measurement>> {
        self.0.into_iter()
    }

    pub(crate) fn get(&self, id: usize) -> Option<&State<Measurement>> {
        self.0.get(id)
    }

    pub(crate) fn loaded(&self) -> impl Iterator<Item = &Measurement> {
        self.0.iter().filter_map(State::loaded)
    }

    pub(crate) fn push(&mut self, measurement: State<Measurement>) {
        self.0.push(measurement);
    }

    pub(crate) fn remove(&mut self, id: usize) -> State<Measurement> {
        self.0.remove(id)
    }

    pub(crate) fn get_loaded(&self, id: Id) -> Option<&Measurement> {
        self.loaded().find(|m| m.id == id)
    }
}

#[derive(Debug)]
pub enum State<Inner, Details = self::Details> {
    NotLoaded(Details),
    Loaded(Inner),
}

#[derive(Debug)]
pub struct Measurement {
    pub id: Id,
    pub details: Details,
    signal: raumklang_core::Measurement,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(usize);

#[derive(Debug, Clone)]
pub struct Details {
    pub name: String,
    pub path: PathBuf,
}

impl<Inner> State<Inner> {
    pub(crate) fn loaded(&self) -> Option<&Inner> {
        match self {
            State::NotLoaded(_) => None,
            State::Loaded(measurement) => Some(measurement),
        }
    }
}

impl<Inner, Details> State<Inner, Details> {
    pub fn as_ref(&self) -> State<&Inner, &Details> {
        match self {
            State::NotLoaded(details) => State::NotLoaded(details),
            State::Loaded(inner) => State::Loaded(inner),
        }
    }
}

impl State<Measurement> {
    pub fn signal(&self) -> Option<slice::Iter<f32>> {
        if let State::Loaded(measurement) = self {
            Some(measurement.signal.iter())
        } else {
            None
        }
    }

    pub fn details(&self) -> &Details {
        match self {
            State::NotLoaded(ref details) => details,
            State::Loaded(ref measurement) => &measurement.details,
        }
    }
}

// impl State<&Measurement, &Details> {
//     pub fn details(&self) -> &Details {
//         match self {
//             State::NotLoaded(ref details) => details,
//             State::Loaded(ref measurement) => &measurement.details,
//         }
//     }
// }

impl Measurement {
    pub fn new(signal: raumklang_core::Measurement, details: Details) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        Self {
            id: Id(ID.fetch_add(1, atomic::Ordering::Relaxed)),
            details,
            signal,
        }
    }
}

impl AsRef<raumklang_core::Measurement> for Measurement {
    fn as_ref(&self) -> &raumklang_core::Measurement {
        &self.signal
    }
}

pub trait FromFile {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized;
}

pub async fn load_from_file<P, T>(path: P) -> Result<T, WavLoadError>
where
    T: FromFile + Send + 'static,
    P: AsRef<Path> + Send + Sync,
{
    let path = path.as_ref().to_owned();
    tokio::task::spawn_blocking(move || T::from_file(path))
        .await
        .map_err(|_err| WavLoadError::Other)?
}

impl FromFile for State<Measurement> {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let details = Details {
            name,
            path: path.to_path_buf(),
        };

        let state = match raumklang_core::Measurement::from_file(path) {
            Ok(data) => State::Loaded(Measurement::new(data, details)),
            Err(_) => State::NotLoaded(details),
        };

        Ok(state)
    }
}
