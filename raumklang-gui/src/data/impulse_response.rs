use std::sync::Arc;

use iced::task::{Sipper, sipper};

#[derive(Debug, Clone, Default)]
pub struct ImpulseResponse(State);

#[derive(Debug, Clone, Default)]
enum State {
    #[default]
    None,
    Computing,
    Computed(Arc<raumklang_core::ImpulseResponse>),
}

impl ImpulseResponse {
    pub fn compute(
        self,
        loopback: &raumklang_core::Loopback,
        measurement: &raumklang_core::Measurement,
    ) -> Option<impl Sipper<Self, Self> + use<>> {
        if let State::Computing = self.0 {
            return None;
        }

        if let State::Computed(_) = self.0 {
            return None;
        }

        let loopback = loopback.clone();
        let measurement = measurement.clone();

        let sipper = sipper(async move |mut progress| {
            progress.send(ImpulseResponse(State::Computing)).await;

            let impulse_response = tokio::task::spawn_blocking(move || {
                raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement)
            })
            .await
            .unwrap()
            .unwrap();

            ImpulseResponse(State::Computed(Arc::new(impulse_response)))
        });

        Some(sipper)
    }

    pub fn result(&self) -> Option<&raumklang_core::ImpulseResponse> {
        match self.0 {
            State::None => None,
            State::Computing => None,
            State::Computed(ref impulse_response) => Some(impulse_response),
        }
    }

    pub fn progress(&self) -> Progress {
        match self.0 {
            State::None => Progress::None,
            State::Computing => Progress::Computing,
            State::Computed(_) => Progress::Computed,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    Computing,
    Computed,
}
