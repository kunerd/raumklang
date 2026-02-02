pub mod pending_window;
pub mod spectral_decay_config;
pub mod spectrogram_config;

pub use pending_window::pending_window;
pub use spectral_decay_config::SpectralDecayConfig;
pub use spectrogram_config::SpectrogramConfig;

#[derive(Default, Debug)]
pub enum Modal {
    #[default]
    None,
    // PendingWindow {
    //     goto_tab: TabId,
    // },
    SpectralDecayConfig(SpectralDecayConfig),
    SpectrogramConfig(SpectrogramConfig),
}
