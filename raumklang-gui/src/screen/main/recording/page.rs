mod component;
pub mod measurement;

pub use component::Page as Component;
pub use measurement::Measurement;

use crate::{
    audio,
    data::{self},
};

use iced::task;

#[derive(Debug, Default)]
pub enum Page {
    #[default]
    Setup,
    LoudnessTest {
        signal_config: data::measurement::Config,
        loudness: audio::Loudness,
        _stream_handle: task::Handle,
    },
    Measurement(Measurement),
}
