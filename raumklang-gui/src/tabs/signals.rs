use std::io::ErrorKind;

use iced::{
    widget::{container, text}, Length, Task,
};
use iced_aw::TabLabel;
use thiserror::Error;

use crate::{
    widgets::chart::{self, TimeSeriesUnit, TimeseriesChart},
    SignalState,
};

use super::Tab;

#[derive(Default)]
pub struct Signals {
    chart: Option<TimeseriesChart>,
}

#[derive(Debug, Clone)]
pub enum SignalsMessage {
    TimeSeriesChart(chart::Message),
    LoopbackSignalSelected,
    MeasurementSignalSelected(usize),
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
    pub fn update(
        &mut self,
        msg: SignalsMessage,
        signals: &mut crate::Signals,
    ) -> Task<SignalsMessage> {
        match msg {
            SignalsMessage::LoopbackSignalSelected => {
                if let Some(SignalState::Loaded(signal)) = &signals.loopback {
                    self.chart = Some(TimeseriesChart::new(signal.clone(), TimeSeriesUnit::Time));
                }
                Task::none()
            }
            SignalsMessage::MeasurementSignalSelected(index) => {
                if let Some(SignalState::Loaded(signal)) = signals.measurements.get(index) {
                    self.chart = Some(TimeseriesChart::new(signal.clone(), TimeSeriesUnit::Time));
                }
                Task::none()
            }
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

