use crate::data::{self, SampleRate};

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
