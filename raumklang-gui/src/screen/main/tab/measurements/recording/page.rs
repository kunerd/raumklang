mod component;

use std::fmt::Display;

pub use component::Page as Component;

use iced::{
    task,
    widget::{column, pick_list, row, text},
};

use crate::{audio, data::measurement};

#[derive(Debug)]
pub enum Page {
    PortSetup,
    LoudnessTest {
        loudness: audio::Loudness,
        _stream_handle: task::Handle,
    },
    MeasurementSetup(ConfigFields),
    MeasurementRunning,
}

impl Default for Page {
    fn default() -> Self {
        Self::PortSetup
    }
}

#[derive(Debug, Clone)]
pub struct ConfigFields {
    pub duration: String,
    pub start_frequency: String,
    pub end_frequency: String,
}

// impl Display for Page {
//     fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
//         let title = match self {
//             Page::PortSetup => "Port Setup",
//             Page::LoudnessTest { .. } => "Loudness Test ...",
//             Page::MeasurementSetup(..) => "Measurement Setup",
//             Page::MeasurementRunning => "Measurement Running ...",
//         };

//         write!(f, "{title}")
//     }
// }

impl From<&measurement::Config> for ConfigFields {
    fn from(config: &measurement::Config) -> Self {
        Self {
            duration: format!("{}", config.duration().as_secs()),
            start_frequency: format!("{}", config.start_frequency()),
            end_frequency: format!("{}", config.end_frequency()),
        }
    }
}
