pub mod pending_window;
pub mod spectral_decay_config;
pub mod spectrogram_config;

pub use pending_window::pending_window;
pub use spectral_decay_config::SpectralDecayConfig;
pub use spectrogram_config::SpectrogramConfig;

use crate::screen::main::{impulse_response::WindowSettings, tab};

#[derive(Default, Debug)]
pub enum Modal {
    #[default]
    None,
    PendingWindow {
        goto_tab: tab::Id,
    },
    SpectralDecayConfig(SpectralDecayConfig),
    SpectrogramConfig(SpectrogramConfig),
}
