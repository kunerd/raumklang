use iced::task::{sipper, Sipper};

#[derive(Debug, Clone)]
pub struct ImpulseResponse(pub raumklang_core::ImpulseResponse);

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

        let inner = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement)
        })
        .await
        .unwrap()
        .unwrap();

        ImpulseResponse(inner)
    })
}

impl AsRef<raumklang_core::ImpulseResponse> for ImpulseResponse {
    fn as_ref(&self) -> &raumklang_core::ImpulseResponse {
        &self.0
    }
}
