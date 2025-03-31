#[derive(Debug)]
pub enum State {
    Computing,
    Computed(FrequencyResponse),
}

#[derive(Debug)]
pub struct FrequencyResponse {
    pub origin: raumklang_core::FrequencyResponse,
}
impl FrequencyResponse {
    pub fn new(origin: raumklang_core::FrequencyResponse) -> Self {
        Self { origin }
    }
}
