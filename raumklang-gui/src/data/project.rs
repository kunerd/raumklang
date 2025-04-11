pub mod file;

use super::{
    frequency_response, impulse_response,
    measurement::{self, loopback},
    Error, Samples, Window,
};
pub use file::File;

use iced::futures::future::join_all;

use std::path::Path;

#[derive(Debug)]
pub struct Project {
    window: Window<Samples>,
    loopback: Option<measurement::Loopback>,
    measurements: Vec<measurement::State>,
}

impl Project {
    pub fn new(
        loopback: Option<measurement::Loopback>,
        measurements: Vec<measurement::State>,
    ) -> Self {
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

    pub fn measurements(&self) -> &[measurement::State] {
        &self.measurements
    }

    pub fn measurements_mut(&mut self) -> &mut [measurement::State] {
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

    pub fn push_measurements(&mut self, measurement: measurement::State) {
        self.measurements.push(measurement);
    }

    pub fn remove_measurement(&mut self, index: usize) -> measurement::State {
        self.measurements.remove(index)
    }

    pub fn set_window(&mut self, window: Window<Samples>) {
        self.window = window;

        self.measurements.iter_mut().for_each(|state| match state {
            measurement::State::NotLoaded(_details) => {}
            measurement::State::Loaded(measurement) => measurement.reset_frequency_responses(),
        });
    }

    fn reset_impulse_responses(&mut self) {
        self.measurements.iter_mut().for_each(|state| match state {
            measurement::State::NotLoaded(_details) => {}
            measurement::State::Loaded(measurement) => measurement.reset_analysis(),
        });
    }

    pub fn impulse_response_computation(
        &mut self,
        id: usize,
    ) -> Result<Option<impulse_response::Computation>, Error> {
        let Some(loopback) = self.loopback.as_ref() else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let Some(measurement) = self.measurements.get_mut(id) else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let loopback::State::Loaded(loopback) = &loopback.state else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let computation = if let measurement::State::Loaded(measurement) = measurement {
            measurement.impulse_response_computation(id, loopback.clone())
        } else {
            None
        };

        Ok(computation)
    }

    pub fn all_frequency_response_computations(
        &mut self,
    ) -> Result<Vec<frequency_response::Computation>, Error> {
        let Some(loopback) = self.loopback.as_ref() else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let loopback::State::Loaded(loopback) = &loopback.state else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        Ok(self
            .measurements
            .iter_mut()
            .enumerate()
            .filter_map(|(id, state)| {
                if let measurement::State::Loaded(measurement) = state {
                    Some((id, measurement))
                } else {
                    None
                }
            })
            .flat_map(|(id, measurement)| match &measurement.analysis {
                measurement::Analysis::None => {
                    let computation = measurement
                        .impulse_response_computation(id, loopback.clone())
                        .unwrap();
                    Some(
                        frequency_response::Computation::from_impulse_response_computation(
                            computation,
                            self.window.clone(),
                        ),
                    )
                }
                measurement::Analysis::ImpulseResponse(impulse_response::State::Computing) => None,
                measurement::Analysis::ImpulseResponse(impulse_response::State::Computed(
                    impulse_response,
                ))
                | measurement::Analysis::FrequencyResponse(impulse_response, _) => {
                    Some(frequency_response::Computation::from_impulse_response(
                        id,
                        impulse_response.clone(),
                        self.window.clone(),
                    ))
                }
            })
            .collect())
    }

    pub fn sample_rate(&self) -> super::SampleRate {
        self.window.sample_rate()
    }
}

impl Default for Project {
    fn default() -> Self {
        let loopback = None;
        let measurements = Vec::new();

        Self::new(loopback, measurements)
    }
}
