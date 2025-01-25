use crate::data;

pub mod measurements;
pub mod impulse_response;
pub mod frequency_response;

pub use measurements::Measurements;
pub use impulse_response::ImpulseResponseTab;
pub use frequency_response::FrequencyResponse;

async fn compute_impulse_response(
    id: data::MeasurementId,
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
) -> (data::MeasurementId, raumklang_core::ImpulseResponse) {
    let impulse_response = tokio::task::spawn_blocking(move || {
        raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement).unwrap()
    })
    .await
    .unwrap();

    (id, impulse_response)
}


