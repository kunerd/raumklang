pub mod file;

pub use file::File;

use super::{
    impulse_response,
    measurement::{self, Loopback},
    Error, Samples, Window,
};

use iced::futures::future::join_all;

use std::path::Path;

#[derive(Debug)]
pub struct Project {
    window: Window<Samples>,
    loopback: Option<measurement::State<Loopback>>,
    pub measurements: measurement::List,
}

impl Project {
    pub fn new(
        loopback: Option<measurement::State<Loopback>>,
        measurements: measurement::List,
    ) -> Self {
        let sample_rate = loopback
            .as_ref()
            .and_then(measurement::State::<Loopback>::sample_rate)
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

        let loopback = match project_file.loopback {
            Some(loopback) => measurement::load_from_file(loopback.path()).await.ok(),
            None => None,
        };

        let measurements = join_all(
            project_file
                .measurements
                .iter()
                .map(|p| measurement::load_from_file(p.path.clone())),
        )
        .await
        .into_iter()
        .flatten();

        Ok(Self::new(
            loopback,
            measurement::List::from_iter(measurements),
        ))
    }

    pub fn window(&self) -> &Window<Samples> {
        &self.window
    }

    pub fn loopback(&self) -> Option<&measurement::State<Loopback>> {
        self.loopback.as_ref()
    }

    pub fn has_no_measurements(&self) -> bool {
        self.loopback.is_none() && self.measurements.is_empty()
    }

    pub fn set_loopback(&mut self, loopback: Option<measurement::State<Loopback>>) {
        let sample_rate = loopback
            .as_ref()
            .and_then(measurement::State::<Loopback>::sample_rate)
            .unwrap_or_default();

        self.window = Window::new(sample_rate).into();
        self.loopback = loopback;

        // self.measurements.clear_analyses();
    }

    pub fn set_window(&mut self, window: Window<Samples>) {
        self.window = window;
        // self.measurements.clear_frequency_responses();
    }

    pub fn impulse_response_computation(
        &self,
        measurement_id: measurement::Id,
    ) -> Result<impulse_response::Computation, Error> {
        let Some(loopback) = self.loopback.as_ref() else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let measurement::State::Loaded(loopback) = &loopback else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        let Some(measurement) = self.measurements.get_loaded(measurement_id) else {
            return Err(Error::ImpulseResponseComputationFailed);
        };

        Ok(impulse_response::Computation::new(
            measurement_id,
            loopback.as_ref().clone(),
            measurement.as_ref().clone(),
        ))
    }

    // pub fn frequency_response_computation(
    //     &mut self,
    //     id: usize,
    // ) -> Result<frequency_response::Computation, Error> {
    //     let Some(loopback) = self.loopback.as_ref() else {
    //         return Err(Error::ImpulseResponseComputationFailed);
    //     };

    //     let measurement::State::Loaded(loopback) = &loopback else {
    //         return Err(Error::ImpulseResponseComputationFailed);
    //     };

    //     let Some(measurement) = self.measurements.get_loaded_mut(id) else {
    //         return Err(Error::ImpulseResponseComputationFailed);
    //     };

    //     let window = self.window.clone();
    //     let computation = if let Some(impulse_response) = measurement.impulse_response().cloned() {
    //         frequency_response::Computation::from_impulse_response(id, impulse_response, window)
    //     } else {
    //         let computation = measurement
    //             .impulse_response_computation(id, loopback.as_ref().clone())
    //             .unwrap();

    //         frequency_response::Computation::from_impulse_response_computation(computation, window)
    //     };

    //     Ok(computation)
    // }
}

impl Default for Project {
    fn default() -> Self {
        Self::new(None, measurement::List::default())
    }
}
