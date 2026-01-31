use crate::data;

#[derive(Debug, Clone, Default)]
pub enum State {
    #[default]
    None,
    Computing,
    Computed(data::Spectrogram),
}

impl State {
    pub fn result(&self) -> Option<&data::Spectrogram> {
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
