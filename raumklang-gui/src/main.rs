use std::{io::ErrorKind, path::Path, sync::Arc};

use iced::{
    executor,
    widget::{button, container, row, text},
    Application, Command, Element, Font, Settings, Subscription, Theme,
};
use thiserror::Error;

struct State {
    loopback_signal: Option<Vec<f32>>,
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
    LoopbackSignalLoaded(Result<Arc<Vec<f32>>, Error>),
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
                Command::perform(pick_file("Loopback"), Message::LoopbackSignalLoaded)
            }
            Message::LoopbackSignalLoaded(result) => match result {
                Ok(data) => {
                    self.loopback_signal = Some(data.to_vec());
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
        let menu = button(text("Load loopback".to_string())).on_press(Message::LoadLoopbackSignal);

        let content = row!(menu);

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

async fn pick_file(file_type: impl AsRef<str>) -> Result<Arc<Vec<f32>>, Error> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title(format!("Choose {} file", file_type.as_ref()))
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)?;

    load_audio_file(handle.path()).await.map_err(Error::File)
}

async fn load_audio_file<P>(path: P) -> Result<Arc<Vec<f32>>, WavLoadError>
where
    P: AsRef<Path> + Send + Sync,
{
    let path = path.as_ref().to_owned();
    tokio::task::spawn_blocking(move || {
        let mut loopback = hound::WavReader::open(path)?;
        // TODO: check the file spec
        // only mono files
        // currently only 32bit float
        loopback
            .samples::<f32>()
            .collect::<hound::Result<Vec<f32>>>()
            .map(Arc::new)
    })
    .await
    .unwrap()
    .map_err(|err| match err {
        hound::Error::IoError(err) => WavLoadError::IoError(err.kind()),
        _ => WavLoadError::Other,
    })
}

fn main() {
    State::run(Settings {
        antialiasing: true,
        default_font: Font::with_name("Noto Sans"),
        ..Settings::default()
    })
    .unwrap();
}
