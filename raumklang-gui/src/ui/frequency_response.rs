use crate::data::{Samples, Window};

use super::{impulse_response, measurement, ImpulseResponse};

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub sample_rate: u32,
    pub data: Vec<f32>,
}

#[derive(Debug)]
pub enum State {
    Computing,
    Computed(FrequencyResponse),
}

impl State {
    pub(crate) fn new(
        id: measurement::Id,
        impulse_response: ImpulseResponse,
        window: Window<Samples>,
    ) -> (Self, Computation) {
        (
            Self::Computing,
            Computation::from_impulse_response(id, impulse_response, window),
        )
    }

    pub(crate) fn from_data(frequency_response: raumklang_core::FrequencyResponse) -> State {
        State::Computed(FrequencyResponse::from_data(frequency_response))
    }
}

impl FrequencyResponse {
    pub fn from_data(frequency_response: raumklang_core::FrequencyResponse) -> Self {
        let sample_rate = frequency_response.sample_rate;
        let data = frequency_response
            .data
            .into_iter()
            .map(|s| s.re.abs())
            .collect();

        Self { sample_rate, data }
    }
}

pub struct Computation {
    id: measurement::Id,
    impulse_response: ImpulseResponse,
    window: Window<Samples>,
}

impl Computation {
    pub fn from_impulse_response(
        id: measurement::Id,
        impulse_response: ImpulseResponse,
        window: Window<Samples>,
    ) -> Self {
        Self {
            id,
            impulse_response,
            window,
        }
    }

    pub async fn run(self) -> (measurement::Id, raumklang_core::FrequencyResponse) {
        let mut impulse_response = self.impulse_response.origin;
        let offset = self.window.offset().into();

        impulse_response.data.rotate_right(offset);

        let window: Vec<_> = self.window.curve().map(|(_x, y)| y).collect();
        let frequency_response = tokio::task::spawn_blocking(move || {
            raumklang_core::FrequencyResponse::new(impulse_response, &window)
        })
        .await
        .unwrap();

        (self.id, frequency_response)
    }
}
