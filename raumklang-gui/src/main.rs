mod widgets;

use std::{io::ErrorKind, path::Path, sync::Arc};

use iced::{
    executor,
    widget::{button, column, container, row, text},
    Application, Command, Element, Font, Length, Settings, Theme,
};
use rfd::FileHandle;
use thiserror::Error;
use widgets::chart::{self, TimeSeriesUnit, TimeseriesChart};

struct State {
    loopback_signal: Option<Signal>,
    measurement_signal: Option<Signal>,
    chart: Option<TimeseriesChart>,
}

impl State {
    fn new() -> Self {
        Self {
            loopback_signal: None,
            measurement_signal: None,
            chart: None,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    LoadLoopbackSignal,
    LoadMeasurementSignal,
    LoopbackSignalLoaded(Result<Arc<Signal>, Error>),
    MeasurementSignalLoaded(Result<Arc<Signal>, Error>),
    LoopbackSignalSelected,
    MeasurementSignalSelected,
    TimeSeriesChart(chart::Message),
}

impl Application for State {
    type Message = self::Message;
    type Executor = executor::Default;
    type Flags = ();
    type Theme = Theme;

    fn new(_flags: Self::Flags) -> (Self, Command<Self::Message>) {
        let app = Self::new();

        (app, Command::none())
    }

    fn title(&self) -> String {
        "Raumklang".to_owned()
    }

    fn update(&mut self, message: Self::Message) -> Command<Self::Message> {
        match message {
            Message::LoadLoopbackSignal => Command::perform(
                pick_file_and_load_signal("loopback"),
                Message::LoopbackSignalLoaded,
            ),
            Message::LoopbackSignalLoaded(result) => match result {
                Ok(signal) => {
                    self.loopback_signal = Arc::into_inner(signal);
                    Command::none()
                }
                Err(err) => {
                    match err {
                        Error::File(reason) => println!("Error: {reason}"),
                        Error::DialogClosed => {}
                    }
                    Command::none()
                }
            },
            Message::LoadMeasurementSignal => Command::perform(
                pick_file_and_load_signal("measurement"),
                Message::MeasurementSignalLoaded,
            ),
            Message::MeasurementSignalLoaded(result) => match result {
                Ok(signal) => {
                    self.measurement_signal = Arc::into_inner(signal);
                    Command::none()
                }
                Err(err) => {
                    println!("{:?}", err);
                    Command::none()
                }
            },
            Message::LoopbackSignalSelected => {
                if let Some(signal) = &self.loopback_signal {
                    self.chart = Some(TimeseriesChart::new(signal.clone(), TimeSeriesUnit::Time));
                }
                Command::none()
            }
            Message::MeasurementSignalSelected => {
                if let Some(signal) = &self.measurement_signal {
                    self.chart = Some(TimeseriesChart::new(signal.clone(), TimeSeriesUnit::Time));
                }
                Command::none()
            }
            Message::TimeSeriesChart(msg) => {
                if let Some(chart) = &mut self.chart {
                    chart.update_msg(msg);
                }
                Command::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let side_menu = {
            let loopback_entry = {
                let header = text("Loopback");
                let btn = if let Some(signal) = &self.loopback_signal {
                    button(signal.view()).on_press(Message::LoopbackSignalSelected)
                } else {
                    button(text("load ...".to_string())).on_press(Message::LoadLoopbackSignal)
                }
                .style(iced::theme::Button::Secondary);

                column!(header, btn).width(Length::Fill).spacing(5)
            };

            let measurement_entry = {
                let header = text("Measurements");
                let btn = if let Some(signal) = &self.measurement_signal {
                    button(signal.view()).on_press(Message::MeasurementSignalSelected)
                } else {
                    button(text("load ...".to_string())).on_press(Message::LoadMeasurementSignal)
                }
                .style(iced::theme::Button::Secondary);

                column!(header, btn).width(Length::Fill).spacing(5)
            };

            container(column!(loopback_entry, measurement_entry).spacing(10))
                .padding(5)
                .width(Length::FillPortion(1))
        };

        let right_container = if let Some(chart) = &self.chart {
            container(chart.view().map(Message::TimeSeriesChart)).width(Length::FillPortion(5))
        } else {
            container(text("TODO".to_string()))
        };

        let right_container = right_container.width(Length::FillPortion(4));

        let content = row!(side_menu, right_container);
        content.into()
    }
}

#[derive(Debug, Clone)]
struct Signal {
    name: String,
    sample_rate: u32,
    data: Vec<f32>,
}

impl Signal {
    pub fn view(&self) -> Element<Message> {
        let samples = self.data.len();
        let sample_rate = self.sample_rate as f32;
        column!(
            text(&self.name),
            text(format!("Samples: {}", samples)),
            text(format!("Duration: {} s", samples as f32 / sample_rate)),
        )
        .padding(2)
        .into()
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let name = path
            .as_ref()
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let mut loopback = hound::WavReader::open(path).map_err(map_hound_error)?;
        let sample_rate = loopback.spec().sample_rate;
        // only mono files
        // currently only 32bit float
        let data = loopback
            .samples::<f32>()
            .collect::<hound::Result<Vec<f32>>>()
            .map_err(map_hound_error)?;

        Ok(Self {
            name,
            sample_rate,
            data,
        })
    }
}

fn map_hound_error(err: hound::Error) -> WavLoadError {
    match err {
        hound::Error::IoError(err) => WavLoadError::IoError(err.kind()),
        _ => WavLoadError::Other,
    }
}

#[derive(Error, Debug, Clone)]
enum WavLoadError {
    #[error("couldn't read file")]
    IoError(ErrorKind),
    #[error("unknown")]
    Other,
}

#[derive(Debug, Clone)]
enum Error {
    File(WavLoadError),
    DialogClosed,
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

fn main() -> iced::Result {
    State::run(Settings {
        antialiasing: true,
        default_font: Font::with_name("Noto Sans"),
        ..Settings::default()
    })
}
