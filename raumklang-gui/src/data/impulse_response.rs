use iced::task::{sipper, Sipper};

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub origin: raumklang_core::ImpulseResponse,
}

impl ImpulseResponse {
    pub fn from_data(impulse_response: raumklang_core::ImpulseResponse) -> Self {
        Self {
            origin: impulse_response,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    ComputationStarted,
}

pub fn compute(
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
) -> impl Sipper<ImpulseResponse, Event> {
    sipper(move |mut output| async move {
        output.send(Event::ComputationStarted).await;

        let impulse_response = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement)
        })
        .await
        .unwrap()
        .unwrap();

        ImpulseResponse::from_data(impulse_response)
    })
}
