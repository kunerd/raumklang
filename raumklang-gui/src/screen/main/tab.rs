use iced::widget::canvas;

use crate::screen::main::{impulse_response::WindowSettings, recording::Recording};

pub enum Tab {
    Measurements { recording: Option<Recording> },
    ImpulseResponses { window_settings: WindowSettings },
    FrequencyResponses { cache: canvas::Cache },
    SpectralDecays { cache: canvas::Cache },
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

impl Default for Tab {
    fn default() -> Self {
        Self::Measurements { recording: None }
    }
}
