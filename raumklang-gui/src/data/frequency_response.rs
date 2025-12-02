use std::sync::Arc;

use super::{Samples, Window};

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub sample_rate: u32,
    pub data: Arc<Vec<f32>>,
}

impl FrequencyResponse {
    pub fn from_data(frequency_response: raumklang_core::FrequencyResponse) -> Self {
        let sample_rate = frequency_response.sample_rate;
        let data = frequency_response
            .data
            .into_iter()
            .map(|s| s.re.abs())
            .collect();

        Self {
            sample_rate,
            data: Arc::new(data),
        }
    }
}

pub async fn compute(
    mut impulse_response: raumklang_core::ImpulseResponse,
    window: Window<Samples>,
) -> FrequencyResponse {
    let offset = window.offset().into();

    impulse_response.data.rotate_right(offset);

    let window: Vec<_> = window.curve().map(|(_x, y)| y).collect();

    tokio::task::spawn_blocking(move || {
        raumklang_core::FrequencyResponse::new(impulse_response, &window)
    })
    .await
    .map(FrequencyResponse::from_data)
    .unwrap()
}
