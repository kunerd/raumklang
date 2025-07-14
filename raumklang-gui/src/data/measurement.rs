pub mod config;
pub mod loopback;

pub use config::Config;
pub use loopback::Loopback;

use super::{frequency_response, impulse_response, FrequencyResponse, ImpulseResponse};

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

    pub(crate) fn iter_mut(&mut self) -> impl Iterator<Item = &mut State<Measurement>> {
        self.0.iter_mut()
    }

    pub(crate) fn get(&self, id: usize) -> Option<&State<Measurement>> {
        self.0.get(id)
    }

    pub(crate) fn get_mut(&mut self, id: usize) -> Option<&mut State<Measurement>> {
        self.0.get_mut(id)
    }

    pub(crate) fn loaded(&self) -> impl Iterator<Item = &Measurement> {
        self.0.iter().filter_map(State::loaded)
    }

    pub(crate) fn loaded_mut(&mut self) -> impl Iterator<Item = &mut Measurement> {
        self.0.iter_mut().filter_map(State::loaded_mut)
    }

    pub(crate) fn push(&mut self, measurement: State<Measurement>) {
        self.0.push(measurement);
    }

    pub(crate) fn remove(&mut self, id: usize) -> State<Measurement> {
        self.0.remove(id)
    }

    // pub(crate) fn clear_frequency_responses(&mut self) {
    //     self.loaded_mut()
    //         .for_each(Measurement::reset_frequency_response);
    // }

    // pub(crate) fn clear_analyses(&mut self) {
    //     self.loaded_mut().for_each(Measurement::reset_analysis);
    // }

    pub(crate) fn get_loaded(&self, id: usize) -> Option<&Measurement> {
        self.loaded().find(|m| m.id == id)
    }

    pub(crate) fn get_loaded_mut(&mut self, id: usize) -> Option<&mut Measurement> {
        self.loaded_mut().find(|m| m.id == id)
    }
}

#[derive(Debug)]
pub enum State<Inner> {
    NotLoaded(Details),
    Loaded(Inner),
}

#[derive(Debug)]
pub struct Measurement {
    pub id: usize,
    pub details: Details,
    signal: raumklang_core::Measurement,
    // pub analysis: Analysis,
}

#[derive(Debug)]
pub enum Analysis {
    None,
    ImpulseResponse(impulse_response::State),
    FrequencyResponse(ImpulseResponse, frequency_response::State),
}

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

    pub(crate) fn loaded_mut(&mut self) -> Option<&mut Inner> {
        match self {
            State::NotLoaded(_) => None,
            State::Loaded(measurement) => Some(measurement),
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
            State::NotLoaded(details) => details,
            State::Loaded(measurement) => &measurement.details,
        }
    }
}

impl Measurement {
    pub fn new(signal: raumklang_core::Measurement, details: Details) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);

        Self {
            id: ID.fetch_add(1, atomic::Ordering::Relaxed),
            details,
            signal,
            // analysis: Analysis::None,
        }
    }

    // pub fn impulse_response(&self) -> Option<&super::ImpulseResponse> {
    //     match &self.analysis {
    //         Analysis::None => None,
    //         Analysis::ImpulseResponse(impulse_response::State::Computing) => None,
    //         Analysis::ImpulseResponse(impulse_response::State::Computed(impulse_response))
    //         | Analysis::FrequencyResponse(impulse_response, _) => Some(impulse_response),
    //     }
    // }

    // pub fn impulse_response_computation(
    //     &mut self,
    //     id: usize,
    //     loopback: raumklang_core::Loopback,
    // ) -> Option<impulse_response::Computation> {
    //     match self.analysis {
    //         Analysis::None => {
    //             self.analysis = Analysis::ImpulseResponse(impulse_response::State::Computing);

    //             Some(impulse_response::Computation::new(
    //                 id,
    //                 loopback,
    //                 self.signal.clone(),
    //             ))
    //         }
    //         Analysis::ImpulseResponse(_) => None,
    //         Analysis::FrequencyResponse(_, _) => None,
    //     }
    // }

    // pub fn impulse_response_computed(&mut self, impulse_response: ImpulseResponse) {
    //     self.analysis =
    //         // Analysis::ImpulseResponse(impulse_response::State::Computed(impulse_response))
    //     Analysis::FrequencyResponse(impulse_response, frequency_response::State::Computing)
    // }

    // pub fn frequency_response(&self) -> Option<&FrequencyResponse> {
    //     let state = self.frequency_response_state()?;

    //     match state {
    //         frequency_response::State::Computing => None,
    //         frequency_response::State::Computed(frequency_response) => Some(frequency_response),
    //     }
    // }

    // pub fn frequency_response_mut(&mut self) -> Option<&mut FrequencyResponse> {
    //     let state = match &mut self.analysis {
    //         Analysis::None => None,
    //         Analysis::ImpulseResponse(_state) => None,
    //         Analysis::FrequencyResponse(_impulse_response, state) => Some(state),
    //     }?;

    //     match state {
    //         frequency_response::State::Computing => None,
    //         frequency_response::State::Computed(frequency_response) => Some(frequency_response),
    //     }
    // }

    // pub fn frequency_response_state(&self) -> Option<&frequency_response::State> {
    //     match &self.analysis {
    //         Analysis::None => None,
    //         Analysis::ImpulseResponse(_state) => None,
    //         Analysis::FrequencyResponse(_impulse_response, state) => Some(state),
    //     }
    // }

    // pub fn frequency_response_computed(&mut self, frequency_response: FrequencyResponse) {
    //     let analysis = std::mem::replace(&mut self.analysis, Analysis::None);

    //     self.analysis = match analysis {
    //         Analysis::None => Analysis::None,
    //         Analysis::ImpulseResponse(impulse_response::State::Computing) => {
    //             Analysis::ImpulseResponse(impulse_response::State::Computing)
    //         }
    //         Analysis::ImpulseResponse(impulse_response::State::Computed(impulse_response))
    //         | Analysis::FrequencyResponse(impulse_response, _) => Analysis::FrequencyResponse(
    //             impulse_response,
    //             frequency_response::State::Computed(frequency_response),
    //         ),
    //     }
    // }

    // pub fn reset_analysis(&mut self) {
    //     self.analysis = Analysis::None
    // }

    // pub fn reset_frequency_response(&mut self) {
    //     let analysis = std::mem::replace(&mut self.analysis, Analysis::None);

    //     self.analysis = match analysis {
    //         Analysis::None => Analysis::None,
    //         Analysis::ImpulseResponse(state) => Analysis::ImpulseResponse(state),
    //         Analysis::FrequencyResponse(impulse_response, _) => {
    //             Analysis::ImpulseResponse(impulse_response::State::Computed(impulse_response))
    //         }
    //     }
    // }
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
