use std::{future::Future, mem};

use raumklang_core::{Loopback, Measurement};

use crate::{
    data,
    ui::{frequency_response, impulse_response, FrequencyResponse, ImpulseResponse},
};

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub impulse_response: impulse_response::State,
    pub frequency_response: FrequencyResponse,
}

impl Analysis {
    pub(crate) fn impulse_response(&self) -> Option<&ImpulseResponse> {
        self.impulse_response.result()
    }

    pub(crate) fn frequency_response_mut(&mut self) -> &mut FrequencyResponse {
        &mut self.frequency_response
    }
}
