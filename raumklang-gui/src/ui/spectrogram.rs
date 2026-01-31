use std::future::Future;

use crate::data::{self, spectrogram};

#[derive(Debug, Clone, Default)]
pub struct Spectrogram(State);

#[derive(Debug, Clone, Default)]
enum State {
    #[default]
    None,
    WaitingForImpulseResponse,
    Computing,
    Computed(data::Spectrogram),
}

impl Spectrogram {
    pub fn result(&self) -> Option<&data::Spectrogram> {
        let State::Computed(ref data) = self.0 else {
            return None;
        };

        Some(data)
    }

    pub fn progress(&self) -> Progress {
        match self.0 {
            State::None => Progress::None,
            State::WaitingForImpulseResponse => Progress::ComputingImpulseResponse,
            State::Computing => Progress::Computing,
            State::Computed(_) => Progress::Finished,
        }
    }

    pub fn compute(
        &mut self,
        impulse_response: &super::impulse_response::State,
        config: &spectrogram::Preferences,
    ) -> Option<impl Future<Output = data::Spectrogram>> {
        if self.result().is_some() {
            return None;
        }

        if let Some(impulse_response) = impulse_response.result() {
            self.0 = State::Computing;

            let computation =
                data::spectrogram::compute(impulse_response.data.clone(), config.clone());

            Some(computation)
        } else {
            self.0 = State::WaitingForImpulseResponse;
            None
        }
    }

    pub fn set_result(&mut self, spectrogram: data::Spectrogram) {
        self.0 = State::Computed(spectrogram);
    }
}

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    ComputingImpulseResponse,
    Computing,
    Finished,
}
