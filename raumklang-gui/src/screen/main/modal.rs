pub mod pending_window;
pub mod save_project;
pub mod spectral_decay_config;
pub mod spectrogram_config;

pub use pending_window::pending_window;
pub use spectral_decay_config::SpectralDecayConfig;
pub use spectrogram_config::SpectrogramConfig;

use crate::screen::main::{recording::Recording, tab};

#[derive(Default, Debug)]
pub enum Modal {
    #[default]
    None,
    PendingWindow {
        goto_tab: tab::Id,
    },
    SpectralDecayConfig(SpectralDecayConfig),
    SpectrogramConfig(SpectrogramConfig),
    // TODO move recording into mod modal
    Recording(Recording),
    SaveProjectDialog(save_project::View),
}
