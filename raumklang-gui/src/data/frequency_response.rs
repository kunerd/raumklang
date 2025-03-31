use super::{impulse_response, ImpulseResponse};

use iced::task::Sipper;

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub origin: raumklang_core::FrequencyResponse,
}

#[derive(Debug)]
pub enum State {
    Computing,
    Computed(FrequencyResponse),
}

pub struct Computation {
    from: CompputationType,
    window: Vec<f32>,
}

enum CompputationType {
    ImpulseResponse(usize, ImpulseResponse),
    Computation(impulse_response::Computation),
}

impl Computation {
    pub fn from_impulse_response(
        id: usize,
        impulse_response: ImpulseResponse,
        window: Vec<f32>,
    ) -> Self {
        Self {
            from: CompputationType::ImpulseResponse(id, impulse_response),
            window,
        }
    }

    pub fn from_impulse_response_computation(
        computation: impulse_response::Computation,
        window: Vec<f32>,
    ) -> Self {
        Self {
            from: CompputationType::Computation(computation),
            window,
        }
    }

    pub fn run(self) -> impl Sipper<(usize, FrequencyResponse), (usize, ImpulseResponse)> {
        iced::task::sipper(async move |mut progress| {
            let (id, impulse_response) = match self.from {
                CompputationType::ImpulseResponse(id, impulse_response) => (id, impulse_response),
                CompputationType::Computation(computation) => computation.run().await.unwrap(),
            };

            progress.send((id, impulse_response.clone())).await;

            let impulse_response = impulse_response.origin;
            let frequency_response = tokio::task::spawn_blocking(move || {
                raumklang_core::FrequencyResponse::new(impulse_response, &self.window)
            })
            .await
            .unwrap();

            (id, FrequencyResponse::new(frequency_response))
        })
    }
}

impl FrequencyResponse {
    pub fn new(origin: raumklang_core::FrequencyResponse) -> Self {
        Self { origin }
    }
}
