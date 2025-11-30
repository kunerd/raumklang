use crate::data::{self, SampleRate};

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub sample_rate: SampleRate,
    pub data: Vec<f32>,
    pub origin: raumklang_core::ImpulseResponse,
}

impl ImpulseResponse {
    pub fn from_data(impulse_response: data::ImpulseResponse) -> Self {
        let max = impulse_response
            .origin
            .data
            .iter()
            .map(|s| s.re.abs())
            .max_by(f32::total_cmp)
            .unwrap();

        let normalized = impulse_response
            .origin
            .data
            .iter()
            .map(|s| s.re)
            .map(|s| s / max.abs())
            .collect();

        Self {
            sample_rate: SampleRate::new(impulse_response.origin.sample_rate),
            data: normalized,
            origin: impulse_response.origin,
        }
    }
}
