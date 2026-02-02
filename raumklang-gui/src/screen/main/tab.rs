use iced::widget::canvas;

use crate::data::Window;

#[derive(Default)]
pub enum Tab {
    #[default]
    Measurements,
    ImpulseResponses {
        pending_window: Window,
    },
    FrequencyResponses {
        cache: canvas::Cache,
    },
    SpectralDecays {
        cache: canvas::Cache,
    },
    Spectrograms,
}

#[derive(Debug, Clone, Copy)]
pub enum Id {
    Measurements,
    ImpulseResponses,
    FrequencyResponses,
    SpectralDecays,
    Spectrograms,
}
