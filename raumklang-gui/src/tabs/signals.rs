use std::io::ErrorKind;

use iced::{
    widget::{container, text},
    Length, Task,
};
use iced_aw::TabLabel;
use thiserror::Error;

use crate::{
    widgets::chart::{self, SignalChart, TimeSeriesUnit},
    Signal,
};

use super::Tab;

#[derive(Default)]
pub struct Signals {
    chart: Option<SignalChart>,
}

#[derive(Debug, Clone)]
pub enum SignalsMessage {
    TimeSeriesChart(chart::SignalChartMessage),
}

#[derive(Debug, Clone)]
pub enum Error {
    File(WavLoadError),
    DialogClosed,
}

#[derive(Error, Debug, Clone)]
pub enum WavLoadError {
    #[error("couldn't read file")]
    IoError(ErrorKind),
    #[error("unknown")]
    Other,
}

impl Signals {
    pub fn selected_signal_changed(&mut self, signal: Signal) {
        self.chart = Some(SignalChart::new(signal, TimeSeriesUnit::Time));
    }

    pub fn update(&mut self, msg: SignalsMessage) -> Task<SignalsMessage> {
        match msg {
            SignalsMessage::TimeSeriesChart(msg) => {
                if let Some(chart) = &mut self.chart {
                    chart.update_msg(msg);
                }
                Task::none()
            }
        }
    }
}

impl Tab for Signals {
    type Message = SignalsMessage;

    fn title(&self) -> String {
        "Signals".to_string()
    }

    fn label(&self) -> iced_aw::TabLabel {
        TabLabel::Text(self.title())
    }

    fn content(&self) -> iced::Element<'_, Self::Message> {
        let content = {
            if let Some(chart) = &self.chart {
                container(chart.view().map(SignalsMessage::TimeSeriesChart))
                    .width(Length::FillPortion(5))
            } else {
                container(text("Not implemented.".to_string()))
            }
        };

        content.into()
    }
}
