use crate::data;

#[derive(Debug, Clone, Default)]
pub enum State {
    #[default]
    None,
    Computing,
    Computed(data::Spectrogram),
}

impl State {
    pub(crate) fn apply(&mut self, event: data::spectrogram::Event) {
        match event {
            data::spectrogram::Event::ComputingStarted => *self = State::Computing,
        }
    }

    pub(crate) fn computed(&mut self, data: data::Spectrogram) {
        *self = State::Computed(data)
    }

    pub(crate) fn result(&self) -> Option<&data::Spectrogram> {
        let State::Computed(data) = self else {
            return None;
        };

        Some(data)
    }
}

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    ComputingImpulseResponse,
    Computing,
    Finished,
}
