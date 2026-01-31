mod impulse_response;

pub use impulse_response::ImpulseResponses;
pub use impulse_response::WindowSettings;

use crate::screen::main::recording::Recording;

use iced::widget::canvas;

pub enum Tab {
    Measurements { recording: Option<Recording> },
    ImpulseResponses { window_settings: WindowSettings },
    FrequencyResponses { cache: canvas::Cache },
    SpectralDecay,
    Spectrogram,
}
