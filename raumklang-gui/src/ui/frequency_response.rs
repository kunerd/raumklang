#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub sample_rate: u32,
    pub data: Vec<f32>,
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
