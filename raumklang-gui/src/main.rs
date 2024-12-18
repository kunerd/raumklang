mod components;
mod tabs;
mod widgets;
mod window;

use std::{
    collections::VecDeque,
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
    measurements::{Error, WavLoadError},
    Tab,
};

#[derive(Debug, Clone)]
enum Message {
    NewProject,
    LoadProject,
    ProjectLoaded(Result<(Measurements, PathBuf), PickAndLoadError>),
    SaveProject,
    ProjectSaved(Result<PathBuf, PickAndSaveError>),
    LoadLoopbackMeasurement,
    LoopbackMeasurementLoaded(Result<Arc<Measurement>, Error>),
    LoadMeasurement,
    MeasurementLoaded(Result<Arc<Measurement>, Error>),
    TabSelected(TabId),
    MeasurementSelected(SelectedMeasurement),
    MeasurementsTab(tabs::measurements::Message),
    ImpulseResponseTab(tabs::impulse_response::Message),
    Debug,
    LoadRecentProject(usize),
    RecentProjectsLoaded(Result<VecDeque<PathBuf>, ()>),
}

enum Raumklang {
    Loading,
    Loaded(State),
}

#[derive(Default)]
struct State {
    measurements: Measurements,
    recent_projects: VecDeque<PathBuf>,
    selected_measurement: Option<SelectedMeasurement>,
    active_tab: TabId,
    measurements_tab: tabs::Measurements,
    impulse_response_tab: tabs::ImpulseResponseTab,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
struct Measurements {
    loopback: Option<MeasurementState>,
    measurements: Vec<MeasurementState>,
}

#[derive(Debug, Clone)]
enum SelectedMeasurement {
    Loopback,
    Measurement(usize),
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum TabId {
    #[default]
    Measurements,
    ImpulseResponse,
}

#[derive(Debug, Clone)]
enum MeasurementState {
    NotLoaded(OfflineMeasurement),
    Loaded(Measurement),
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct OfflineMeasurement {
    name: String,
    path: PathBuf,
}

#[derive(Debug, Clone)]
struct Measurement {
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
    iced::application(Raumklang::title, Raumklang::update, Raumklang::view)
        .default_font(Font::with_name("Noto Sans"))
        .antialiasing(true)
        .run_with(Raumklang::new)
}

impl Raumklang {
    fn new() -> (Self, Task<Message>) {
        (
            Self::Loading,
            Task::perform(load_recent_projects(), Message::RecentProjectsLoaded),
        )
    }

    fn title(&self) -> String {
        const APPLICATION_NAME: &str = "Raumklang";
        let additional_name = match self {
            Raumklang::Loading => "- loading ...",
            Raumklang::Loaded(_) => "",
        };

        format!("{APPLICATION_NAME} {additional_name}").to_string()
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match self {
            Raumklang::Loading => {
                match message {
                    Message::RecentProjectsLoaded(Ok(recent_projects)) => {
                        *self = Self::Loaded(State {
                            recent_projects,
                            ..State::default()
                        })
                    }
                    Message::RecentProjectsLoaded(Err(_)) => *self = Self::Loaded(State::default()),
                    _ => {}
                }

                Task::none()
            }
            Raumklang::Loaded(state) => state.update(message),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            Raumklang::Loading => text("Application is loading").into(),
            Raumklang::Loaded(state) => state.view(),
        }
    }
}

impl State {
    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::RecentProjectsLoaded(_) => Task::none(),
            Message::TabSelected(id) => {
                self.active_tab = id;
                Task::none()
            }
            Message::MeasurementsTab(msg) => self
                .measurements_tab
                .update(msg)
                .map(Message::MeasurementsTab),
            Message::ImpulseResponseTab(msg) => self
                .impulse_response_tab
                .update(msg)
                .map(Message::ImpulseResponseTab),
            Message::NewProject => {
                *self = Self {
                    recent_projects: self.recent_projects.clone(),
                    ..Self::default()
                };
                Task::none()
            }
            Message::LoadProject => Task::perform(pick_file_and_load(), Message::ProjectLoaded),
            Message::LoadRecentProject(id) => {
                let Some(recent) = self.recent_projects.get(id) else {
                    return Task::none();
                };

                Task::perform(
                    load_project_from_file(recent.clone()),
                    Message::ProjectLoaded,
                )
            }
            Message::SaveProject => {
                let content = serde_json::to_string_pretty(&self.measurements).unwrap();
                Task::perform(pick_file_and_save(content), Message::ProjectSaved)
            }
            Message::ProjectLoaded(res) => match &res {
                Ok((signals, _)) => {
                    self.measurements = Measurements::default();
                    let mut tasks = vec![];
                    if let Some(MeasurementState::NotLoaded(signal)) = &signals.loopback {
                        let path = signal.path.clone();
                        tasks.push(Task::perform(
                            async {
                                load_signal_from_file(path)
                                    .await
                                    .map(Arc::new)
                                    .map_err(Error::File)
                            },
                            Message::LoopbackMeasurementLoaded,
                        ));
                    }

                    for m in &signals.measurements {
                        if let MeasurementState::NotLoaded(signal) = m {
                            let path = signal.path.clone();
                            tasks.push(Task::perform(
                                async {
                                    load_signal_from_file(path)
                                        .await
                                        .map(Arc::new)
                                        .map_err(Error::File)
                                },
                                Message::MeasurementLoaded,
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
            Message::ProjectSaved(res) => match res {
                Ok(path) => {
                    if self.recent_projects.contains(&path) {
                        return Task::none();
                    }

                    self.recent_projects.push_front(path);
                    let recent = self.recent_projects.clone();
                    Task::perform(async move { save_recent_projects(&recent).await }, |_| {})
                        .discard()
                }
                Err(err) => {
                    println!("{err}");
                    Task::none()
                }
            },
            Message::LoadLoopbackMeasurement => Task::perform(
                pick_file_and_load_signal("loopback"),
                Message::LoopbackMeasurementLoaded,
            ),
            Message::LoopbackMeasurementLoaded(result) => match result {
                Ok(signal) => {
                    if let Some(signal) = Arc::into_inner(signal) {
                        self.measurements.loopback = Some(MeasurementState::Loaded(signal.clone()));
                        self.impulse_response_tab
                            .loopback_signal_changed(signal)
                            .map(Message::ImpulseResponseTab)
                    } else {
                        Task::none()
                    }
                }
                Err(err) => {
                    match err {
                        Error::File(reason) => println!("Error: {reason}"),
                        Error::DialogClosed => {}
                    }
                    Task::none()
                }
            },
            Message::LoadMeasurement => Task::perform(
                pick_file_and_load_signal("measurement"),
                Message::MeasurementLoaded,
            ),
            Message::MeasurementLoaded(result) => match result {
                Ok(signal) => {
                    let signal = Arc::into_inner(signal)
                        .map(MeasurementState::Loaded)
                        .unwrap();
                    self.measurements.measurements.push(signal);
                    Task::none()
                }
                Err(err) => {
                    println!("{:?}", err);
                    Task::none()
                }
            },
            Message::MeasurementSelected(selected) => {
                let task = match (&self.active_tab, selected.clone()) {
                    (TabId::Measurements, SelectedMeasurement::Loopback) => {
                        self.selected_measurement = Some(selected);
                        if let Some(MeasurementState::Loaded(m)) = &self.measurements.loopback {
                            self.measurements_tab.set_measurement(m.clone());
                        }
                        Task::none()
                    }
                    (TabId::ImpulseResponse, SelectedMeasurement::Loopback) => Task::none(),
                    (_, SelectedMeasurement::Measurement(index)) => {
                        self.selected_measurement = Some(selected);
                        if let Some(MeasurementState::Loaded(measurement)) =
                            self.measurements.measurements.get(index)
                        {
                            self.measurements_tab.set_measurement(measurement.clone());
                            self.impulse_response_tab
                                .set_selected_measurement(measurement.clone())
                        } else {
                            Task::none()
                        }
                    }
                };
                task.map(Message::ImpulseResponseTab)
            }
            Message::Debug => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let recent_menu = {
            let recent_entries: Vec<_> = self
                .recent_projects
                .iter()
                .enumerate()
                .map(|(i, r)| {
                    Item::new(
                        button(r.file_name().unwrap().to_str().unwrap())
                            .width(Length::Shrink)
                            .style(button::secondary)
                            .on_press(Message::LoadRecentProject(i)),
                    )
                })
                .collect();

            if recent_entries.is_empty() {
                Item::new(
                    button("Load recent ...")
                        .width(Length::Fill)
                        .style(button::secondary),
                )
            } else {
                Item::with_menu(
                    button("Load recent ...")
                        .width(Length::Fill)
                        .style(button::secondary)
                        .on_press(Message::Debug),
                    Menu::new(recent_entries).width(Length::Shrink),
                )
            }
        };
        let project_menu = Item::with_menu(
            button(text("Project").align_y(Vertical::Center))
                .width(Length::Shrink)
                .style(button::secondary)
                .on_press(Message::Debug),
            Menu::new(
                [
                    Item::new(
                        button("New")
                            .width(Length::Fill)
                            .style(button::secondary)
                            .on_press(Message::NewProject),
                    ),
                    Item::new(
                        button("Load ...")
                            .width(Length::Fill)
                            .style(button::secondary)
                            .on_press(Message::LoadProject),
                    ),
                    recent_menu,
                    Item::new(
                        button("Save ...")
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
                let content: Element<_> = match &self.measurements.loopback {
                    Some(MeasurementState::Loaded(signal)) => {
                        let style = if let Some(SelectedMeasurement::Loopback) =
                            self.selected_measurement
                        {
                            button::primary
                        } else {
                            button::secondary
                        };

                        button(signal_list_entry(signal))
                            .on_press(Message::MeasurementSelected(SelectedMeasurement::Loopback))
                            .style(style)
                            .width(Length::Fill)
                            .into()
                    }
                    Some(MeasurementState::NotLoaded(signal)) => offline_signal_list_entry(signal),
                    None => text("Please load a loopback signal.").into(),
                };

                let add_msg = self
                    .measurements
                    .loopback
                    .as_ref()
                    .map_or(Some(Message::LoadLoopbackMeasurement), |_| None);

                signal_list_category("Loopback", add_msg, content)
            };

            let measurement_entry = {
                let content: Element<_> = {
                    if self.measurements.measurements.is_empty() {
                        text("Please load a measurement.").into()
                    } else {
                        let entries: Vec<Element<_>> = self
                            .measurements
                            .measurements
                            .iter()
                            .enumerate()
                            .map(|(index, state)| match state {
                                MeasurementState::Loaded(signal) => {
                                    let style = match self.selected_measurement {
                                        Some(SelectedMeasurement::Measurement(i)) if i == index => {
                                            button::primary
                                        }
                                        Some(_) => button::secondary,
                                        None => button::secondary,
                                    };
                                    button(signal_list_entry(signal))
                                        .on_press(Message::MeasurementSelected(
                                            SelectedMeasurement::Measurement(index),
                                        ))
                                        .width(Length::Fill)
                                        .style(style)
                                        .into()
                                }
                                MeasurementState::NotLoaded(signal) => {
                                    offline_signal_list_entry(signal)
                                }
                            })
                            .collect();

                        column(entries).padding(5).spacing(5).into()
                    }
                };

                signal_list_category("Measurements", Some(Message::LoadMeasurement), content)
            };

            container(column!(loopback_entry, measurement_entry).spacing(10))
                .padding(5)
                .width(Length::FillPortion(1))
                .into()
        };

        let tabs = Tabs::new(Message::TabSelected)
            .push(
                TabId::Measurements,
                self.measurements_tab.label(),
                self.measurements_tab.view().map(Message::MeasurementsTab),
            )
            .push(
                TabId::ImpulseResponse,
                self.impulse_response_tab.label(),
                self.impulse_response_tab
                    .view()
                    .map(Message::ImpulseResponseTab),
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

impl Measurement {
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

impl serde::Serialize for MeasurementState {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let offline_signal = match self {
            MeasurementState::NotLoaded(signal) => signal,
            MeasurementState::Loaded(signal) => &OfflineMeasurement {
                name: signal.name.clone(),
                path: signal.path.clone(),
            },
        };

        offline_signal.serialize(serializer)
    }
}

impl<'de> serde::Deserialize<'de> for MeasurementState {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let offline_signal = Deserialize::deserialize(deserializer)?;

        Ok(MeasurementState::NotLoaded(offline_signal))
    }
}

impl serde::Serialize for Measurement {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        let unloaded_signal = OfflineMeasurement {
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

fn signal_list_entry(signal: &Measurement) -> Element<'_, Message> {
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

fn offline_signal_list_entry(signal: &OfflineMeasurement) -> Element<'_, Message> {
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

async fn pick_file_and_load() -> Result<(Measurements, PathBuf), PickAndLoadError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Datei mit Kundendaten auswählen...")
        .pick_file()
        .await
        .ok_or(PickAndLoadError::DialogClosed)?;

    load_project_from_file(handle.path()).await
}

async fn load_project_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<(Measurements, PathBuf), PickAndLoadError> {
    //let store = load_from_file(handle.path()).await?;
    let path = path.as_ref();
    let content = tokio::fs::read(path)
        .await
        .map_err(|err| FileError::Io(err.kind()))?;

    let signals =
        serde_json::from_slice(&content).map_err(|err| FileError::Json(err.to_string()))?;

    Ok((signals, path.to_path_buf()))
}

async fn save_to_file(path: PathBuf, content: String) -> Result<(), FileError> {
    tokio::fs::write(path, content)
        .await
        .map_err(|err| FileError::Io(err.kind()))
}

async fn pick_file_and_load_signal(file_type: impl AsRef<str>) -> Result<Arc<Measurement>, Error> {
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

async fn load_signal_from_file<P>(path: P) -> Result<Measurement, WavLoadError>
where
    P: AsRef<Path> + Send + Sync,
{
    let path = path.as_ref().to_owned();
    tokio::task::spawn_blocking(move || Measurement::from_file(path))
        .await
        .unwrap()
}

fn get_app_data_dir() -> PathBuf {
    let app_dirs = directories::ProjectDirs::from("de", "HenKu", "raumklang").unwrap();
    app_dirs.data_local_dir().to_path_buf()
}

fn get_recent_project_file_path<P: AsRef<Path>>(path: P) -> PathBuf {
    const RECENT_PROJECTS_FILE_NAME: &str = "recent_projects.json";

    let mut file_path = path.as_ref().to_path_buf();
    file_path.push(RECENT_PROJECTS_FILE_NAME);
    file_path
}

async fn save_recent_projects(recent_projects: &VecDeque<PathBuf>) {
    let app_data_dir = get_app_data_dir();
    tokio::fs::create_dir_all(&app_data_dir).await.unwrap();

    let file_path = get_recent_project_file_path(app_data_dir);
    let contents = serde_json::to_string_pretty(recent_projects).unwrap();
    tokio::fs::write(file_path, contents).await.unwrap();
}

async fn load_recent_projects() -> Result<VecDeque<PathBuf>, ()> {
    let app_data_dir = get_app_data_dir();
    let file_path = get_recent_project_file_path(app_data_dir);

    let content = tokio::fs::read(file_path).await.map_err(|_| ())?;
    serde_json::from_slice(&content).map_err(|_| ())
}
