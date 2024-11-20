mod tabs;
mod widgets;
mod window;

use std::{
    io,
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{
    alignment::Vertical,
    border::Radius,
    widget::{button, column, container, horizontal_rule, horizontal_space, row, scrollable, text},
    Alignment, Border, Element, Font, Length, Task,
};
use iced_aw::{
    menu::{self, primary, Item},
    style::Status,
    Menu, MenuBar, Tabs,
};

use rfd::FileHandle;
use serde::Deserialize;
use tabs::{
    signals::{Error, WavLoadError},
    Tab,
};

#[derive(Debug, Clone)]
enum Message {
    LoadProject,
    SaveProject,
    ProjectLoaded(Result<(Signals, PathBuf), PickAndLoadError>),
    ProjectSaved(Result<PathBuf, PickAndSaveError>),
    LoadLoopbackSignal,
    LoadMeasurementSignal,
    LoopbackSignalLoaded(Result<Arc<Signal>, Error>),
    MeasurementSignalLoaded(Result<Arc<Signal>, Error>),
    TabSelected(TabId),
    SignalSelected(SelectedSignal),
    SignalsTab(tabs::signals::SignalsMessage),
    ImpulseResponse(tabs::impulse_response::Message),
    Debug,
}

#[derive(Default)]
struct State {
    signals: Signals,
    selected_signal: Option<SelectedSignal>,
    active_tab: TabId,
    signals_tab: tabs::Signals,
    impulse_response_tab: tabs::ImpulseResponse,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Signals {
    loopback: Option<SignalState>,
    measurements: Vec<SignalState>,
}

#[derive(Debug, Clone)]
enum SelectedSignal {
    Loopback,
    Measurement(usize),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum TabId {
    #[default]
    Signals,
    ImpulseResponse,
}

#[derive(Debug, Clone)]
enum SignalState {
    NotLoaded(OfflineSignal),
    Loaded(Signal),
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

fn main() -> iced::Result {
    iced::application(State::title, State::update, State::view)
        .default_font(Font::with_name("Noto Sans"))
        .antialiasing(true)
        .run()
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
            Message::SignalsTab(msg) => self.signals_tab.update(msg).map(Message::SignalsTab),
            Message::ImpulseResponse(msg) => self
                .impulse_response_tab
                .update(msg)
                .map(Message::ImpulseResponse),
            Message::LoadProject => Task::perform(pick_file_and_load(), Message::ProjectLoaded),
            Message::SaveProject => {
                let content = serde_json::to_string_pretty(&self.signals).unwrap();
                Task::perform(pick_file_and_save(content), Message::ProjectSaved)
            }
            Message::ProjectLoaded(res) => match &res {
                Ok((signals, _)) => {
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
                            Message::LoopbackSignalLoaded,
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
                                Message::MeasurementSignalLoaded,
                            ));
                        }
                    }

                    Task::batch(tasks)
                }
                Err(err) => {
                    println!("{err}");
                    Task::none()
                }
            },
            Message::ProjectSaved(res) => {
                println!("{res:?}");
                Task::none()
            }
            Message::LoadLoopbackSignal => Task::perform(
                pick_file_and_load_signal("loopback"),
                Message::LoopbackSignalLoaded,
            ),
            Message::LoopbackSignalLoaded(result) => match result {
                Ok(signal) => {
                    self.signals.loopback = Arc::into_inner(signal).map(SignalState::Loaded);
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
            Message::LoadMeasurementSignal => Task::perform(
                pick_file_and_load_signal("measurement"),
                Message::MeasurementSignalLoaded,
            ),
            Message::MeasurementSignalLoaded(result) => match result {
                Ok(signal) => {
                    let signal = Arc::into_inner(signal).map(SignalState::Loaded).unwrap();
                    self.signals.measurements.push(signal);
                    Task::none()
                }
                Err(err) => {
                    println!("{:?}", err);
                    Task::none()
                }
            },
            Message::Debug => Task::none(),
            Message::SignalSelected(selected) => {
                let task = match selected {
                    SelectedSignal::Loopback => {
                        if let Some(SignalState::Loaded(signal)) = &self.signals.loopback {
                            self.signals_tab.selected_signal_changed(signal.clone());
                            self.impulse_response_tab
                                .loopback_signal_changed(signal.clone())
                        } else {
                            Task::none()
                        }
                    }
                    SelectedSignal::Measurement(index) => {
                        if let Some(SignalState::Loaded(signal)) =
                            self.signals.measurements.get(index)
                        {
                            self.signals_tab.selected_signal_changed(signal.clone());
                            self.impulse_response_tab
                                .measurement_signal_changed(signal.clone())
                        } else {
                            Task::none()
                        }
                    }
                };

                self.selected_signal = Some(selected);
                task.map(Message::ImpulseResponse)
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let project_menu = Item::with_menu(
            button(text("Projekt").align_y(Vertical::Center))
                .width(Length::Shrink)
                .style(button::primary)
                .on_press(Message::Debug),
            Menu::new(
                [
                    Item::new(
                        button("laden...")
                            .width(Length::Fill)
                            .style(button::secondary)
                            .on_press(Message::LoadProject),
                    ),
                    Item::new(
                        button("speichern...")
                            .width(Length::Fill)
                            .style(button::secondary)
                            .on_press(Message::SaveProject),
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
                    radius: Radius::new(3.0),
                    ..Default::default()
                },
                ..primary(theme, status)
            });

        let side_menu: Element<_> = {
            let loopback_entry = {
                let content: Element<_> = match &self.signals.loopback {
                    Some(SignalState::Loaded(signal)) => {
                        let style = if let Some(SelectedSignal::Loopback) = self.selected_signal {
                            button::primary
                        } else {
                            button::secondary
                        };

                        button(signal_list_entry(signal))
                            .on_press(Message::SignalSelected(SelectedSignal::Loopback))
                            .style(style)
                            .width(Length::Fill)
                            .into()
                    }
                    Some(SignalState::NotLoaded(signal)) => offline_signal_list_entry(signal),
                    None => text("Please load a loopback signal.").into(),
                };

                let add_msg = self
                    .signals
                    .loopback
                    .as_ref()
                    .map_or(Some(Message::LoadLoopbackSignal), |_| None);

                signal_list_category("Loopback", add_msg, content)
            };

            let measurement_entry = {
                let content: Element<_> = {
                    if self.signals.measurements.is_empty() {
                        text("Please load a measurement.").into()
                    } else {
                        let entries: Vec<Element<_>> = self
                            .signals
                            .measurements
                            .iter()
                            .enumerate()
                            .map(|(index, state)| match state {
                                SignalState::Loaded(signal) => {
                                    let style = match self.selected_signal {
                                        Some(SelectedSignal::Measurement(i)) if i == index => {
                                            button::primary
                                        }
                                        Some(_) => button::secondary,
                                        None => button::secondary,
                                    };
                                    button(signal_list_entry(signal))
                                        .on_press(Message::SignalSelected(
                                            SelectedSignal::Measurement(index),
                                        ))
                                        .width(Length::Fill)
                                        .style(style)
                                        .into()
                                }
                                SignalState::NotLoaded(signal) => offline_signal_list_entry(signal),
                            })
                            .collect();

                        column(entries).padding(5).spacing(5).into()
                    }
                };

                signal_list_category(
                    "Measurements",
                    Some(Message::LoadMeasurementSignal),
                    content,
                )
            };

            container(column!(loopback_entry, measurement_entry).spacing(10))
                .padding(5)
                .width(Length::FillPortion(1))
                .into()
        };

        let tabs = Tabs::new(Message::TabSelected)
            .push(
                TabId::Signals,
                self.signals_tab.label(),
                self.signals_tab.view().map(Message::SignalsTab),
            )
            .push(
                TabId::ImpulseResponse,
                self.impulse_response_tab.label(),
                self.impulse_response_tab
                    .view()
                    .map(Message::ImpulseResponse),
            )
            .set_active_tab(&self.active_tab)
            .tab_bar_position(iced_aw::TabBarPosition::Top);

        let side_menu = scrollable(side_menu).width(Length::FillPortion(1));
        let r = row!(side_menu, tabs.width(Length::FillPortion(3)));
        let c = column!(menu, r);
        //let sc = scrollable(c);
        let back = container(c).width(Length::Fill).height(Length::Fill);

        back.into()
    }
}

fn signal_list_category<'a>(
    name: &'a str,
    add_msg: Option<Message>,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let header = row!(text(name), horizontal_space()).align_y(Alignment::Center);

    let header = if let Some(msg) = add_msg {
        header.push(button("+").on_press(msg))
    } else {
        header
    };

    column!(header, horizontal_rule(1), content)
        .width(Length::Fill)
        .spacing(5)
        .padding(10)
        .into()
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

fn map_hound_error(err: hound::Error) -> WavLoadError {
    match err {
        hound::Error::IoError(err) => WavLoadError::IoError(err.kind()),
        _ => WavLoadError::Other,
    }
}

fn signal_list_entry(signal: &Signal) -> Element<'_, Message> {
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

fn offline_signal_list_entry(signal: &OfflineSignal) -> Element<'_, Message> {
    column!(text(&signal.name), button("Reload"))
        .padding(2)
        .into()
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
    tokio::fs::write(path, content)
        .await
        .map_err(|err| FileError::Io(err.kind()))
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
