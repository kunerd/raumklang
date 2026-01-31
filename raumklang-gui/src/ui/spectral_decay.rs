use std::future::Future;

use iced::Task;

use crate::{
    data,
    ui::{self, impulse_response},
    Message,
};

#[derive(Debug, Clone, Default)]
pub struct SpectralDecay(State);

#[derive(Debug, Clone, Default)]
enum State {
    #[default]
    None,
    WaitingForImpulseResponse,
    Computing,
    Computed(data::SpectralDecay),
}

impl SpectralDecay {
    pub fn result(&self) -> Option<&data::SpectralDecay> {
        let State::Computed(result) = &self.0 else {
            return None;
        };

        Some(result)
    }
    pub fn progress(&self) -> Progress {
        match self.0 {
            State::None => Progress::None,
            State::WaitingForImpulseResponse => Progress::WaitingForImpulseResponse,
            State::Computing => Progress::Computing,
            State::Computed(_) => Progress::Finished,
        }
    }

    pub fn compute_spectral_decay(
        &mut self,
        impulse_response: &impulse_response::State,
        config: data::spectral_decay::Config,
    ) -> Option<impl Future<Output = data::SpectralDecay>> {
        if self.result().is_some() {
            return None;
        }

        if let Some(impulse_response) = impulse_response.result() {
            self.0 = ui::spectral_decay::State::Computing;

            let computation = data::spectral_decay::compute(impulse_response.data.clone(), config);

            Some(computation)
        } else {
            self.0 = ui::spectral_decay::State::WaitingForImpulseResponse;
            None
        }
    }

    pub fn set_result(&mut self, spectral_decay: data::SpectralDecay) {
        self.0 = State::Computed(spectral_decay);
    }

    pub fn reset(&mut self) {
        self.0 = State::None
    }
}

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    WaitingForImpulseResponse,
    Computing,
    Finished,
}
