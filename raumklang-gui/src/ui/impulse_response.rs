#[derive(Debug, Default, Clone)]
pub enum State {
    #[default]
    Computing,
    Computed(ImpulseResponse),
}

impl State {
    pub(crate) fn set_computed(&mut self, impulse_response: ImpulseResponse) {
        *self = State::Computed(impulse_response)
    }

    pub fn computed(&self) -> Option<&ImpulseResponse> {
        match self {
            State::Computing => None,
            State::Computed(ref impulse_response) => Some(impulse_response),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub max: f32,
    pub sample_rate: u32,
    pub data: Vec<f32>,
    pub origin: raumklang_core::ImpulseResponse,
}

impl ImpulseResponse {
    pub fn from_data(impulse_response: raumklang_core::ImpulseResponse) -> Self {
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
            origin: impulse_response,
        }
    }
}
