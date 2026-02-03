use crate::ui::{
    FrequencyResponse, ImpulseResponse, impulse_response, spectral_decay::SpectralDecay,
    spectrogram::Spectrogram,
};

#[derive(Debug, Clone, Default)]
pub struct Analysis {
    pub impulse_response: impulse_response::State,
    pub frequency_response: FrequencyResponse,
    pub spectral_decay: SpectralDecay,
    pub spectrogram: Spectrogram,
}

impl Analysis {
    pub(crate) fn impulse_response(&self) -> Option<&ImpulseResponse> {
        self.impulse_response.result()
    }

    pub(crate) fn frequency_response_mut(&mut self) -> &mut FrequencyResponse {
        &mut self.frequency_response
    }
}
