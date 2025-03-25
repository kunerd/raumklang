use super::Measurement;

#[derive(Debug)]
pub struct ImpulseResponse {
    pub name: String,
    measurement_id: Option<usize>,
    state: State,
}

#[derive(Debug, Default)]
pub enum State {
    #[default]
    NotComputed,
    Computed(raumklang_core::ImpulseResponse),
}

impl ImpulseResponse {
    pub fn for_measurement(id: usize, measurement: &Measurement) -> Self {
        ImpulseResponse {
            name: measurement.name.clone(),
            measurement_id: Some(id),
            state: State::NotComputed,
        }
    }
}

async fn compute_impulse_response(
    id: usize,
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
) -> (usize, raumklang_core::ImpulseResponse) {
    let impulse_response = tokio::task::spawn_blocking(move || {
        raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement).unwrap()
    })
    .await
    .unwrap();

    (id, impulse_response)
}
