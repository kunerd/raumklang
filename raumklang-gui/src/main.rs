mod components;
mod tabs;
mod widgets;
mod window;

use std::{
    collections::VecDeque,
    io, mem,
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{
    alignment::Vertical,
    border::Radius,
    widget::{button, column, container, row, text},
    Border, Element, Font, Length, Task,
};
use iced_aw::{
    menu::{self, primary, Item},
    style::Status,
    Menu, MenuBar,
};

use rfd::FileHandle;
use serde::Deserialize;
use tabs::measurements::{self, Error, WavLoadError};

#[derive(Debug, Clone)]
enum Message {
    NewProject,
    LoadProject,
    ProjectLoaded(Result<(Data, PathBuf), PickAndLoadError>),
    SaveProject,
    ProjectSaved(Result<PathBuf, PickAndSaveError>),
    LoopbackMeasurementLoaded(Result<Arc<Measurement>, Error>),
    MeasurementLoaded(Result<Arc<Measurement>, Error>),
    TabSelected(TabId),
    MeasurementsTab(tabs::measurements::Message),
    ImpulseResponseTab(tabs::impulse_response::Message),
    Debug,
    LoadRecentProject(usize),
    RecentProjectsLoaded(Result<VecDeque<PathBuf>, ()>),
}

enum Tab {
    Measurements(tabs::Measurements),
    Analysis(tabs::Measurements, tabs::ImpulseResponseTab),
}

impl Default for Tab {
    fn default() -> Self {
        Self::Measurements(tabs::Measurements::default())
    }
}

enum Raumklang {
    Loading,
    Loaded(State),
}

#[derive(Default)]
struct State {
    active_tab: Tab,
    data: DataState,
    recent_projects: VecDeque<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
enum DataState {
    NotEnoughMeasurements {
        loopback: Option<MeasurementState>,
        measurements: Vec<MeasurementState>,
    },
    Measurements(Data),
}

enum DataStateChange {
    LoopbackAdded(MeasurementState),
    MeasurementAdded(MeasurementState),
}

impl DataState {
    pub fn loopback(&self) -> Option<&MeasurementState> {
        match self {
            DataState::NotEnoughMeasurements { loopback, .. } => loopback.as_ref(),
            DataState::Measurements(data) => Some(&data.loopback),
        }
    }

    pub fn measurements(&self) -> &Vec<MeasurementState> {
        match self {
            DataState::NotEnoughMeasurements { measurements, .. } => measurements,
            DataState::Measurements(data) => &data.measurements,
        }
    }

    fn transition(&mut self, action: DataStateChange) {
        let cur_state = mem::take(self);
        let next_state = match (cur_state, action) {
            (
                DataState::NotEnoughMeasurements {
                    mut loopback,
                    mut measurements,
                },
                action,
            ) => {
                match action {
                    DataStateChange::LoopbackAdded(m) => loopback = Some(m),
                    DataStateChange::MeasurementAdded(m) => measurements.push(m),
                }

                match loopback {
                    Some(loopback) if !measurements.is_empty() => Self::Measurements(Data {
                        loopback,
                        measurements: measurements.to_vec(),
                    }),
                    _ => Self::NotEnoughMeasurements {
                        loopback,
                        measurements: measurements.to_vec(),
                    },
                }
            }
            (DataState::Measurements(mut data), DataStateChange::LoopbackAdded(loopback)) => {
                data.loopback = loopback;
                Self::Measurements(data)
            }
            (DataState::Measurements(mut data), DataStateChange::MeasurementAdded(m)) => {
                data.measurements.push(m);
                Self::Measurements(data)
            }
        };

        *self = next_state;
    }
}

impl Default for DataState {
    fn default() -> Self {
        Self::NotEnoughMeasurements {
            loopback: None,
            measurements: vec![],
        }
    }
}
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
struct Data {
    loopback: MeasurementState,
    measurements: Vec<MeasurementState>,
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
    Loaded(Arc<Measurement>),
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
                let cur_tab = mem::take(&mut self.active_tab);
                self.active_tab = match (id, cur_tab) {
                    (TabId::Measurements, _) => Tab::Measurements(tabs::Measurements::default()),
                    (TabId::ImpulseResponse, Tab::Measurements(m_tab)) => {
                        Tab::Analysis(m_tab, tabs::ImpulseResponseTab::default())
                    }
                    (TabId::ImpulseResponse, tab) => tab,
                };
                Task::none()
            }
            Message::MeasurementsTab(msg) => {
                let Tab::Measurements(tab) = &mut self.active_tab else {
                    return Task::none();
                };

                let (task, event) = tab.update(msg, &self.data);

                let event_task = match event {
                    Some(measurements::Event::LoadLoopbackMeasurement) => Task::perform(
                        pick_file_and_load_signal("loopback"),
                        Message::LoopbackMeasurementLoaded,
                    ),
                    Some(measurements::Event::LoadMeasurement) => Task::perform(
                        pick_file_and_load_signal("measurement"),
                        Message::MeasurementLoaded,
                    ),
                    None => Task::none(),
                };

                Task::batch(vec![event_task, task.map(Message::MeasurementsTab)])
            }
            Message::ImpulseResponseTab(msg) => {
                //self
                //.impulse_response_tab
                //.update(msg)
                //.map(Message::ImpulseResponseTab),
                Task::none()
            }
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
                let content = serde_json::to_string_pretty(&self.data).unwrap();
                Task::perform(pick_file_and_save(content), Message::ProjectSaved)
            }
            Message::ProjectLoaded(res) => match &res {
                Ok((signals, _)) => {
                    self.data = DataState::default();
                    let mut tasks = vec![];
                    if let MeasurementState::NotLoaded(signal) = &signals.loopback {
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
            Message::LoopbackMeasurementLoaded(result) => match result {
                Ok(signal) => {
                    self.data
                        .transition(DataStateChange::LoopbackAdded(MeasurementState::Loaded(
                            signal,
                        )));
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
            Message::MeasurementLoaded(result) => match result {
                Ok(signal) => {
                    self.data
                        .transition(DataStateChange::MeasurementAdded(MeasurementState::Loaded(signal)));
                    Task::none()
                }
                Err(err) => {
                    println!("{:?}", err);
                    Task::none()
                }
            },
            Message::Debug => Task::none(),
        }
    }

    fn view(&self) -> Element<'_, Message> {
        let menu = {
            let project_menu = {
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

                Item::with_menu(
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
                )
            };

            MenuBar::new(vec![project_menu])
                .draw_path(menu::DrawPath::Backdrop)
                .style(|theme: &iced::Theme, status: Status| menu::Style {
                    path_border: Border {
                        radius: Radius::new(3.0),
                        ..Default::default()
                    },
                    ..primary(theme, status)
                })
        };

        let content = {
            let ir_button_msg = match self.data {
                DataState::NotEnoughMeasurements { .. } => None,
                DataState::Measurements(_) => Some(Message::TabSelected(TabId::ImpulseResponse)),
            };

            fn tab_button(title: &str, active: bool, msg: Option<Message>) -> Element<'_, Message> {
                let btn = button(title).on_press_maybe(msg);
                match active {
                    true => btn.style(button::primary),
                    false => btn.style(button::secondary),
                }
                .into()
            }

            let tab_bar = row![
                tab_button(
                    "Measurements",
                    matches!(self.active_tab, Tab::Measurements(_)),
                    Some(Message::TabSelected(TabId::Measurements))
                ),
                tab_button(
                    "Impulse Response",
                    matches!(self.active_tab, Tab::Analysis(..)),
                    ir_button_msg
                )
            ];

            let tab_content = match &self.active_tab {
                Tab::Measurements(tab) => tab.view(&self.data).map(Message::MeasurementsTab),
                Tab::Analysis(_, ir) => ir.view().map(Message::ImpulseResponseTab),
            };

            column!(tab_bar, tab_content)
        };
        let c = column!(menu, content);
        //let sc = scrollable(c);
        let back = container(c).width(Length::Fill).height(Length::Fill);

        back.into()
    }
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

async fn pick_file_and_load() -> Result<(Data, PathBuf), PickAndLoadError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Datei mit Kundendaten auswählen...")
        .pick_file()
        .await
        .ok_or(PickAndLoadError::DialogClosed)?;

    load_project_from_file(handle.path()).await
}

async fn load_project_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<(Data, PathBuf), PickAndLoadError> {
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
