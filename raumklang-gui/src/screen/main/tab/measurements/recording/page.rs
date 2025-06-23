mod component;
pub mod signal_setup;

pub use component::Page as Component;
pub use signal_setup::SignalSetup;

use crate::{
    audio,
    data::{measurement, recording::port},
};

use iced::{
    task,
    widget::{column, pick_list, row, text},
};

#[derive(Debug)]
pub enum Page {
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
    MeasurementRunning {
        finished_len: usize,
        loudness: audio::Loudness,
        data: Vec<f32>,
    },
}

impl Default for Page {
    fn default() -> Self {
        Self::PortSetup
    }
}
