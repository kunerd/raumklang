use std::{io::ErrorKind, path::Path, sync::Arc};

use iced::{
    executor,
    widget::{button, column, container, row, text},
    Application, Command, Element, Font, Length, Settings, Subscription, Theme,
};
use rfd::FileHandle;
use thiserror::Error;

struct State {
    loopback_signal: Option<Signal>,
}

impl State {
    fn new() -> Self {
        Self {
            loopback_signal: None,
        }
    }
}

#[derive(Debug, Clone)]
enum Message {
    LoadLoopbackSignal,
    LoopbackSignalLoaded(Result<Arc<Signal>, Error>),
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
            Message::LoadLoopbackSignal => {
                Command::perform(pick_file_and_load_signal("Loopback"), Message::LoopbackSignalLoaded)
            }
            Message::LoopbackSignalLoaded(result) => match result {
                Ok(signal) => {
                    self.loopback_signal = Arc::into_inner(signal);
                    Command::none()
                }
                Err(err) => {
                    println!("{:?}", err);
                    Command::none()
                }
            },
        }
    }

    fn view(&self) -> Element<'_, Self::Message> {
        let menu =
            row!(button(text("Load loopback".to_string())).on_press(Message::LoadLoopbackSignal));

        let mut signal_list = vec![];

        if let Some(signal) = &self.loopback_signal {
            let samples = signal.data.len();
            let sample_rate = signal.sample_rate as f32;
            let loopback_entry = column!(
                text(&signal.name),
                text(format!("Length: {}", samples)),
                text(format!("Duration: {}", samples as f32 / sample_rate)),
            );
            //let loopback_entry = text("Loopback Signal".to_string());
            signal_list.push(loopback_entry.into());
        }

        let left_container = container(column(signal_list)).width(Length::FillPortion(1));
        let right_container = container(text("TODO".to_string())).width(Length::FillPortion(5));

        let content = column!(menu, row!(left_container, right_container));
        container(content).into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        //subscription::events()
        //    .map(TimeSeriesMessage::EventOccured)
        //    .map(ImpulseResponseMessage::TimeSeries)
        //    .map(Message::ImpulseRespone)
        Subscription::none()
    }
}

#[derive(Debug)]
struct Signal {
    name: String,
    sample_rate: u32,
    data: Vec<f32>,
}

impl Signal {
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

fn main() {
    State::run(Settings {
        antialiasing: true,
        default_font: Font::with_name("Noto Sans"),
        ..Settings::default()
    })
    .unwrap();
}
