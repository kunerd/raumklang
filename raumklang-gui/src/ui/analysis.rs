use std::{future::Future, mem};

use raumklang_core::{Loopback, Measurement};

use crate::{
    data,
    ui::{frequency_response, impulse_response, FrequencyResponse, ImpulseResponse},
};

#[derive(Debug, Clone, Default)]
pub struct Analysis(State);

pub enum Event {
    ImpulseResponseComputed(ImpulseResponse),
    FrequencyResponseComputed(data::FrequencyResponse),
}

impl Analysis {
    pub(crate) fn apply(&mut self, event: Event) {
        match event {
            Event::ImpulseResponseComputed(impulse_response) => {
                let State::ImpulseResponseComputing(frequency_response) = mem::take(&mut self.0)
                else {
                    return;
                };

                self.0 = State::ImpulseResponse {
                    impulse_response: impulse_response,
                    frequency_response,
                }
            }
            Event::FrequencyResponseComputed(fr) => {
                let State::ImpulseResponse {
                    ref mut frequency_response,
                    ..
                } = self.0
                else {
                    return;
                };

                frequency_response.computed(fr);
            }
        }
    }

    pub(crate) fn impulse_response(&self) -> Option<&ImpulseResponse> {
        let State::ImpulseResponse {
            ref impulse_response,
            ..
        } = self.0
        else {
            return None;
        };

        Some(impulse_response)
    }

    pub(crate) fn frequency_response(&self) -> Option<&FrequencyResponse> {
        match &self.0 {
            State::None => None,
            State::ImpulseResponseComputing(frequency_response)
            | State::ImpulseResponse {
                frequency_response, ..
            } => Some(frequency_response),
        }
    }

    pub(crate) fn impulse_response_progress(&self) -> impulse_response::Progress {
        match self.0 {
            State::None => impulse_response::Progress::None,
            State::ImpulseResponseComputing(_) => impulse_response::Progress::Computing,
            State::ImpulseResponse { .. } => impulse_response::Progress::Finished,
        }
    }

    pub(crate) fn compute_impulse_response(
        &mut self,
        loopback: Loopback,
        measurement: Measurement,
    ) -> Option<impl Future<Output = data::ImpulseResponse>> {
        match self.0 {
            State::ImpulseResponse { .. } => None,
            State::ImpulseResponseComputing(_) => None,
            State::None => {
                self.0 = State::ImpulseResponseComputing(FrequencyResponse::new());

                Some(data::impulse_response::compute(loopback, measurement))
            }
        }
    }

    pub(crate) fn compute_frequency_response(
        &mut self,
        window: data::Window<data::Samples>,
    ) -> Option<impl Future<Output = data::FrequencyResponse>> {
        let State::ImpulseResponse {
            ref impulse_response,
            ref mut frequency_response,
        } = self.0
        else {
            return None;
        };

        if let frequency_response::Progress::Finished = frequency_response.progress {
            return None;
        }

        frequency_response.progress = frequency_response::Progress::Computing;

        Some(data::frequency_response::compute(
            impulse_response.origin.clone(),
            window,
        ))
    }
}

#[derive(Debug, Clone, Default)]
enum State {
    #[default]
    None,
    ImpulseResponseComputing(FrequencyResponse),
    ImpulseResponse {
        impulse_response: ImpulseResponse,
        frequency_response: FrequencyResponse,
    },
}
