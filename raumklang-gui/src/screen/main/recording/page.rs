mod component;
pub mod measurement;
pub mod signal_setup;

pub use component::Page as Component;
pub use measurement::Measurement;
pub use signal_setup::SignalSetup;

use crate::{audio, data::recording::port};

use iced::task;

#[derive(Debug, Default)]
pub enum Page {
    #[default]
    PortSetup,
    LoudnessTest {
        config: port::Config,
        loudness: audio::Loudness,
        _stream_handle: task::Handle,
    },
    SignalSetup {
        config: port::Config,
        page: signal_setup::SignalSetup,
    },
    Measurement(Measurement),
}
