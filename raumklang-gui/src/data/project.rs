pub mod file;

use super::{
    impulse_response,
    measurement::{self, loopback},
    Error, Measurement, Samples, Window,
};
pub use file::File;

use iced::futures::future::join_all;
use raumklang_core::{FrequencyResponse, ImpulseResponse};

use std::path::Path;

#[derive(Debug)]
pub struct Project {
    window: Window<Samples>,
    loopback: Option<measurement::Loopback>,
    measurements: Vec<Measurement>,
}

pub struct ImpulseResponseComputation {
    id: usize,
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
}

impl Project {
    pub fn new(loopback: Option<measurement::Loopback>, measurements: Vec<Measurement>) -> Self {
        let sample_rate = loopback
            .as_ref()
            .and_then(measurement::Loopback::sample_rate)
            .unwrap_or_default();

        let window = Window::new(sample_rate).into();

        Self {
            window,
            loopback,
            measurements,
        }
    }

    pub async fn load(path: impl AsRef<Path>) -> Result<Self, file::Error> {
        let path = path.as_ref();
        let project_file = File::load(path).await?;

        let loopback: Option<measurement::Loopback> = match project_file.loopback {
            Some(loopback) => measurement::load_from_file(loopback.path()).await.ok(),
            None => None,
        };

        let measurements: Vec<_> = join_all(
            project_file
                .measurements
                .iter()
                .map(|p| measurement::load_from_file(p.path.clone())),
        )
        .await
        .into_iter()
        .flatten()
        .collect();

        Ok(Self::new(loopback, measurements))
    }

    pub fn window(&self) -> &Window<Samples> {
        &self.window
    }

    pub fn loopback(&self) -> Option<&measurement::Loopback> {
        self.loopback.as_ref()
    }

    pub fn measurements(&self) -> &[Measurement] {
        &self.measurements
    }

    pub fn measurements_mut(&mut self) -> &mut [Measurement] {
        &mut self.measurements
    }

    pub fn has_no_measurements(&self) -> bool {
        self.loopback.is_none() && self.measurements.is_empty()
    }

    pub fn set_loopback(&mut self, loopback: Option<measurement::Loopback>) {
        let sample_rate = loopback
            .as_ref()
            .and_then(measurement::Loopback::sample_rate)
            .unwrap_or_default();

        self.window = Window::new(sample_rate).into();
        self.loopback = loopback;

        self.reset_impulse_responses();
    }

    pub fn push_measurements(&mut self, measurement: Measurement) {
        self.measurements.push(measurement);
    }

    pub fn remove_measurement(&mut self, index: usize) -> Measurement {
        self.measurements.remove(index)
    }

    pub fn set_window(&mut self, window: Window<Samples>) {
        self.window = window;
    }

    fn reset_impulse_responses(&mut self) {
        self.measurements
            .iter_mut()
            .for_each(|m| match &mut m.state {
                measurement::State::NotLoaded => {}
                measurement::State::Loaded {
                    impulse_response, ..
                } => *impulse_response = impulse_response::State::NotComputed,
            });
    }
}

impl Default for Project {
    fn default() -> Self {
        let loopback = None;
        let measurements = Vec::new();

        Self::new(loopback, measurements)
    }
}

impl ImpulseResponseComputation {
    pub fn new(measurement_id: usize, project: &mut Project) -> Result<Option<Self>, Error> {
        let Some(loopback) = project.loopback.as_ref() else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let Some(measurement) = project.measurements.get_mut(measurement_id) else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let loopback::State::Loaded(loopback) = &loopback.state else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let measurement::State::Loaded {
            data: measurement,
            impulse_response: impulse_response @ impulse_response::State::NotComputed,
        } = &mut measurement.state
        else {
            return Ok(None);
        };

        *impulse_response = impulse_response::State::Computing;

        let loopback = loopback.clone();
        let measurement = measurement.clone();

        Ok(Some(ImpulseResponseComputation {
            id: measurement_id,
            loopback,
            measurement,
        }))
    }

    pub async fn run(self) -> Result<(usize, super::ImpulseResponse), Error> {
        let id = self.id;

        let impulse_response = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&self.loopback, &self.measurement)
                .unwrap()
        })
        .await
        .unwrap();

        Ok((id, impulse_response.into()))
    }
}
