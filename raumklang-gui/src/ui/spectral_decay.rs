use crate::data;

#[derive(Debug, Clone, Default)]
pub enum State {
    #[default]
    None,
    Computing,
    Computed(data::SpectralDecay),
}

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    ComputingImpulseResponse,
    Computing,
    Finished,
}

impl State {
    pub(crate) fn computed(&mut self, decay: data::SpectralDecay) {
        *self = State::Computed(decay)
    }

    pub(crate) fn result(&self) -> Option<&data::SpectralDecay> {
        let State::Computed(spectral_decay) = self else {
            return None;
        };

        Some(spectral_decay)
    }
}
