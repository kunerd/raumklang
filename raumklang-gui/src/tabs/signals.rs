use std::{io::ErrorKind, path::Path, sync::Arc};

use iced::{
    widget::{button, column, container, row, text},
    Element, Length, Task,
};
use iced_aw::TabLabel;
use rfd::FileHandle;
use thiserror::Error;

use crate::{
    widgets::chart::{self, TimeSeriesUnit, TimeseriesChart},
    Signal,
};

use super::Tab;

#[derive(Default)]
pub struct Signals {
    chart: Option<TimeseriesChart>,
    loopback_signal: Option<Signal>,
    measurement_signal: Option<Signal>,
}

#[derive(Debug, Clone)]
pub enum SignalsMessage {
    TimeSeriesChart(chart::Message),
    LoadLoopbackSignal,
    LoadMeasurementSignal,
    LoopbackSignalLoaded(Result<Arc<Signal>, Error>),
    MeasurementSignalLoaded(Result<Arc<Signal>, Error>),
    LoopbackSignalSelected,
    MeasurementSignalSelected,
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
    pub fn update(&mut self, msg: SignalsMessage) -> Task<SignalsMessage> {
        match msg {
            SignalsMessage::LoadLoopbackSignal => Task::perform(
                pick_file_and_load_signal("loopback"),
                SignalsMessage::LoopbackSignalLoaded,
            ),
            SignalsMessage::LoopbackSignalLoaded(result) => match result {
                Ok(signal) => {
                    self.loopback_signal = Arc::into_inner(signal);
                    Task::none()
                }
                Err(err) => {
                    match err {
                        Error::File(reason) => println!("Error: {reason}"),
                        Error::DialogClosed => {}
                    }
                    Task::none()
                }
            },
            SignalsMessage::LoadMeasurementSignal => Task::perform(
                pick_file_and_load_signal("measurement"),
                SignalsMessage::MeasurementSignalLoaded,
            ),
            SignalsMessage::MeasurementSignalLoaded(result) => match result {
                Ok(signal) => {
                    self.measurement_signal = Arc::into_inner(signal);
                    Task::none()
                }
                Err(err) => {
                    println!("{:?}", err);
                    Task::none()
                }
            },
            SignalsMessage::LoopbackSignalSelected => {
                if let Some(signal) = &self.loopback_signal {
                    self.chart = Some(TimeseriesChart::new(signal.clone(), TimeSeriesUnit::Time));
                }
                Task::none()
            }
            SignalsMessage::MeasurementSignalSelected => {
                if let Some(signal) = &self.measurement_signal {
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
        let side_menu: Element<'_, SignalsMessage> = {
            let loopback_entry = {
                let header = text("Loopback");
                let btn = if let Some(signal) = &self.loopback_signal {
                    button(signal_list_entry(signal)).on_press(SignalsMessage::LoopbackSignalSelected)
                } else {
                    button(text("load ...".to_string()))
                        .on_press(SignalsMessage::LoadLoopbackSignal)
                }
                .style(button::secondary);

                column!(header, btn).width(Length::Fill).spacing(5)
            };

            let measurement_entry = {
                let header = text("Measurements");
                let btn = if let Some(signal) = &self.measurement_signal {
                    button(signal_list_entry(signal)).on_press(SignalsMessage::MeasurementSignalSelected)
                } else {
                    button(text("load ...".to_string()))
                        .on_press(SignalsMessage::LoadMeasurementSignal)
                }
                .style(button::secondary);

                column!(header, btn).width(Length::Fill).spacing(5)
            };

            container(column!(loopback_entry, measurement_entry).spacing(10))
                .padding(5)
                .width(Length::FillPortion(1))
                .into()
        };

        let content = {
            if let Some(chart) = &self.chart {
                container(chart.view().map(SignalsMessage::TimeSeriesChart))
                    .width(Length::FillPortion(5))
            } else {
                container(text("Not implemented.".to_string()))
            }
        };

        row!(side_menu, content).into()
    }
}

fn signal_list_entry(signal: &Signal) -> Element<'_, SignalsMessage> {
    let samples = signal.data.len();
    let sample_rate = signal.sample_rate as f32;
    column!(
        text(&signal.name),
        text(format!("Samples: {}", samples)),
        text(format!("Duration: {} s", samples as f32 / sample_rate)),
    )
    .padding(2)
    .into()
}

async fn pick_file_and_load_signal(file_type: impl AsRef<str>) -> Result<Arc<Signal>, Error> {
    let handle = pick_file(file_type).await?;
    load_signal_from_file(handle.path())
        .await
        .map(Arc::new)
        .map_err(Error::File)
}

async fn pick_file(file_type: impl AsRef<str>) -> Result<FileHandle, Error> {
    rfd::AsyncFileDialog::new()
        .set_title(format!("Choose {} file", file_type.as_ref()))
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)
}

async fn load_signal_from_file<P>(path: P) -> Result<Signal, WavLoadError>
where
    P: AsRef<Path> + Send + Sync,
{
    let path = path.as_ref().to_owned();
    tokio::task::spawn_blocking(move || Signal::from_file(path))
        .await
        .unwrap()
}
