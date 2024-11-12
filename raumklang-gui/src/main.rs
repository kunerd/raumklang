mod tabs;
mod widgets;
mod window;

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{
    border::Radius,
    widget::{button, column, container},
    Border, Element, Font, Length, Task,
};
use iced_aw::{
    menu::{self, primary, Item},
    style::Status,
    Menu, MenuBar, Tabs,
};

use serde::Deserialize;
use tabs::{
    signals::{load_signal_from_file, Error, SignalsMessage, WavLoadError},
    Tab,
};

#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum TabId {
    #[default]
    Signals,
    ImpulseResponse,
}

#[derive(Default)]
struct State {
    signals: Signals,
    active_tab: TabId,
    signals_tab: tabs::Signals,
    impulse_response_tab: tabs::ImpulseResponse,
}

#[derive(Debug, Clone)]
enum SignalState {
    NotLoaded(OfflineSignal),
    Loaded(Signal),
}

impl serde::Serialize for SignalState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let offline_signal = match self {
            SignalState::NotLoaded(signal) => signal,
            SignalState::Loaded(signal) => &OfflineSignal {
                name: signal.name.clone(),
                path: signal.path.clone(),
            },
        };

        offline_signal.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for SignalState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let offline_signal = Deserialize::deserialize(deserializer)?;

        Ok(SignalState::NotLoaded(offline_signal))
    }
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Signals {
    loopback: Option<SignalState>,
    measurements: Vec<SignalState>,
}

#[derive(Debug, Clone)]
enum Message {
    TabSelected(TabId),
    SignalsTab(tabs::signals::SignalsMessage),
    ImpulseResponse(tabs::impulse_response::Message),
    Load,
    Save,
    SignalsLoaded(Result<(Signals, PathBuf), PickAndLoadError>),
    SignalsSaved(Result<PathBuf, PickAndSaveError>),
}

impl State {
    fn title(&self) -> String {
        "Raumklang".to_owned()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(id) => {
                self.active_tab = id;
                Task::none()
            }
            Message::SignalsTab(msg) => self
                .signals_tab
                .update(msg, &mut self.signals)
                .map(Message::SignalsTab),
            Message::ImpulseResponse(msg) => self
                .impulse_response_tab
                .update(msg, &self.signals)
                .map(Message::ImpulseResponse),
            Message::Load => Task::perform(pick_file_and_load(), Message::SignalsLoaded),
            Message::Save => {
                let content = serde_json::to_string_pretty(&self.signals).unwrap();
                Task::perform(pick_file_and_save(content), Message::SignalsSaved)
            }
            Message::SignalsLoaded(res) => {
                match &res {
                    Ok((signals, _)) => {
                        self.signals = signals.clone();

                        let mut tasks = vec![];
                        if let Some(SignalState::NotLoaded(signal)) = &signals.loopback {
                            let path = signal.path.clone();
                            tasks.push(Task::perform(
                                async {
                                    load_signal_from_file(path)
                                        .await
                                        .map(Arc::new)
                                        .map_err(Error::File)
                                },
                                SignalsMessage::LoopbackSignalLoaded,
                            ));
                        }

                        for m in &signals.measurements {
                            if let SignalState::NotLoaded(signal) = m {
                                let path = signal.path.clone();
                                tasks.push(Task::perform(
                                    async {
                                        load_signal_from_file(path)
                                            .await
                                            .map(Arc::new)
                                            .map_err(Error::File)
                                    },
                                    SignalsMessage::MeasurementSignalLoaded,
                                ));
                            }
                        }

                        Task::batch(tasks).map(Message::SignalsTab)
                    }
                    Err(err) => {
                        println!("{err}");
                        Task::none()
                    }
                }
            }
            Message::SignalsSaved(res) => {
                println!("{res:?}");
                Task::none()
            },
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let project_menu = Item::with_menu(
            button("Projekt").width(Length::Shrink),
            Menu::new(
                [
                    Item::new(
                        button("laden...")
                            .width(Length::Fill)
                            .on_press(Message::Load),
                    ),
                    Item::new(
                        button("speichern...")
                            .width(Length::Fill)
                            .on_press(Message::Save),
                    ),
                ]
                .into(),
            )
            .width(180),
        );

        let menu = MenuBar::new(vec![project_menu])
            .draw_path(menu::DrawPath::Backdrop)
            .style(|theme: &iced::Theme, status: Status| menu::Style {
                path_border: Border {
                    radius: Radius::new(6.0),
                    ..Default::default()
                },
                ..primary(theme, status)
            });

        let tabs = Tabs::new(Message::TabSelected)
            .push(
                TabId::Signals,
                self.signals_tab.label(),
                self.signals_tab
                    .view(&self.signals)
                    .map(Message::SignalsTab),
            )
            .push(
                TabId::ImpulseResponse,
                self.impulse_response_tab.label(),
                self.impulse_response_tab
                    .view(&self.signals)
                    .map(Message::ImpulseResponse),
            )
            .set_active_tab(&self.active_tab)
            .tab_bar_position(iced_aw::TabBarPosition::Top);

        let c = column!(menu, tabs);
        //let sc = scrollable(c);
        let back = container(c).width(Length::Fill).height(Length::Fill);

        back.into()
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct OfflineSignal {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct Signal {
    name: String,
    path: PathBuf,
    sample_rate: u32,
    data: Vec<f32>,
}

impl serde::Serialize for Signal {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let unloaded_signal = OfflineSignal {
            name: self.name.clone(),
            path: self.path.clone(),
        };

        unloaded_signal.serialize(serializer)
    }
}

impl Signal {
    pub fn new(name: String, sample_rate: u32, data: Vec<f32>) -> Self {
        Self {
            name,
            path: PathBuf::new(),
            sample_rate,
            data,
        }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let name = path
            .as_ref()
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let mut loopback = hound::WavReader::open(path.as_ref()).map_err(map_hound_error)?;
        let sample_rate = loopback.spec().sample_rate;
        // only mono files
        // currently only 32bit float
        let data = loopback
            .samples::<f32>()
            .collect::<hound::Result<Vec<f32>>>()
            .map_err(map_hound_error)?;

        Ok(Self {
            name,
            path: path.as_ref().to_path_buf(),
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

fn main() -> iced::Result {
    iced::application(State::title, State::update, State::view)
        //.subscription(State::subscription)
        .default_font(Font::with_name("Noto Sans"))
        .antialiasing(true)
        .run()
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum FileError {
    #[error("Fehler beim laden der Datei: {0}")]
    Io(io::ErrorKind),
    #[error("Dateiinhalt fehlerhaft: {0}")]
    Json(String),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PickAndLoadError {
    #[error("Dateiauswahl wurde geschlossen")]
    DialogClosed,
    #[error(transparent)]
    File(#[from] FileError),
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PickAndSaveError {
    #[error("Dateiauswahl wurde geschlossen")]
    DialogClosed,
    #[error(transparent)]
    File(#[from] FileError),
}

async fn pick_file_and_save(content: String) -> Result<PathBuf, PickAndSaveError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Speicherort wählen ...")
        .save_file()
        .await
        .ok_or(PickAndSaveError::DialogClosed)?;

    let path = handle.path().to_path_buf();
    save_to_file(path.clone(), content).await?;

    Ok(path)
}

async fn pick_file_and_load() -> Result<(Signals, PathBuf), PickAndLoadError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Datei mit Kundendaten auswählen...")
        .pick_file()
        .await
        .ok_or(PickAndLoadError::DialogClosed)?;

    //let store = load_from_file(handle.path()).await?;
    let content = tokio::fs::read(handle.path())
        .await
        .map_err(|err| FileError::Io(err.kind()))?;

    let signals =
        serde_json::from_slice(&content).map_err(|err| FileError::Json(err.to_string()))?;

    Ok((signals, handle.path().to_path_buf()))
}

async fn save_to_file(path: PathBuf, content: String) -> Result<(), FileError> {
    //log::debug!("Save store to file: {path:?}");

    tokio::fs::write(path, content)
        .await
        .map_err(|err| FileError::Io(err.kind()))
}
