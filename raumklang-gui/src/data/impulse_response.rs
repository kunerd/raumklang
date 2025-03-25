#[derive(Debug, Default, Clone)]
pub enum State {
    #[default]
    NotComputed,
    Computing,
    Computed(ImpulseResponse),
}

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub max: f32,
    pub sample_rate: u32,
    pub data: Vec<f32>,
}

impl From<raumklang_core::ImpulseResponse> for ImpulseResponse {
    fn from(impulse_response: raumklang_core::ImpulseResponse) -> Self {
        let data: Vec<_> = impulse_response
            .data
            .iter()
            .map(|s| s.re.powi(2).sqrt())
            .collect();

        let max = data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));

        Self {
            max,
            sample_rate: impulse_response.sample_rate,
            data,
        }
    }
}
