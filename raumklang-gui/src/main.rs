mod components;
mod data;
mod tabs;
mod widgets;
//mod window;

use std::{
    collections::HashMap,
    io, mem,
    path::{Path, PathBuf},
    sync::Arc,
};

use iced::{
    alignment::Vertical,
    border::Radius,
    widget::{button, column, container, row, text},
    Border, Element, Font, Length, Settings, Task, Theme,
};
use iced_aw::{
    menu::{self, primary, Item},
    style::Status,
    Menu, MenuBar,
};

use data::{FromFile, Project, ProjectLoopback, ProjectMeasurement};
use raumklang_core::WavLoadError;
use rfd::FileHandle;
use tabs::{
    impulse_response,
    measurements::{self, Error},
};

const MAX_RECENT_PROJECTS_ENTRIES: usize = 10;

#[derive(Debug, Clone)]
enum Message {
    NewProject,
    LoadProject,
    ProjectLoaded(Result<(data::Project, PathBuf), PickAndLoadError>),
    SaveProject,
    ProjectSaved(Result<PathBuf, PickAndSaveError>),
    LoopbackMeasurementLoaded(Result<Arc<data::Loopback>, Error>),
    MeasurementLoaded(Result<Arc<data::Measurement>, Error>),
    TabSelected(TabId),
    MeasurementsTab(tabs::measurements::Message),
    ImpulseResponseTab(tabs::impulse_response::Message),
    Debug,
    LoadRecentProject(usize),
    RecentProjectsLoaded(Result<data::RecentProjects, ()>),
}

enum Tab {
    Measurements(tabs::Measurements),
    Analysis(tabs::ImpulseResponseTab),
}

impl Default for Tab {
    fn default() -> Self {
        Self::Measurements(tabs::Measurements::default())
    }
}

#[derive(Default)]
#[allow(clippy::large_enum_variant)]
enum Raumklang {
    #[default]
    Loading,
    Loaded {
        active_tab: Tab,
        loopback: Option<data::MeasurementState<data::Loopback, OfflineMeasurement>>,
        measurements: data::Store<data::Measurement, OfflineMeasurement>,
        impulse_responses: HashMap<usize, raumklang_core::ImpulseResponse>,
        recent_projects: data::RecentProjects,
    },
}

#[derive(Debug, Default, Clone, PartialEq, Eq)]
enum TabId {
    #[default]
    Measurements,
    ImpulseResponse,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OfflineMeasurement {
    name: String,
    path: PathBuf,
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
        .theme(Raumklang::theme)
        .settings(Settings {
            fonts: vec![include_bytes!("../fonts/raumklang-icons.ttf")
                .as_slice()
                .into()],
            ..Default::default()
        })
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
            Raumklang::Loaded { .. } => "",
        };

        format!("{APPLICATION_NAME} {additional_name}").to_string()
    }

    fn theme(&self) -> Theme {
        Theme::KanagawaDragon
    }

    fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::NewProject => {
                let Raumklang::Loaded { measurements, .. } = self else {
                    return Task::none();
                };

                *measurements = data::Store::new();

                Task::none()
            }
            Message::LoadProject => Task::perform(pick_file_and_load(), Message::ProjectLoaded),
            Message::ProjectLoaded(Ok((project, path))) => {
                let Raumklang::Loaded {
                    measurements,
                    recent_projects,
                    ..
                } = self
                else {
                    return Task::none();
                };

                let mut tasks = vec![];

                recent_projects.insert(path);
                let recent_projects = recent_projects.clone();
                tasks.push(
                    Task::perform(
                        async move { save_recent_projects(&recent_projects).await },
                        |_| {},
                    )
                    .discard(),
                );

                *measurements = data::Store::new();
                if let Some(loopback) = project.loopback {
                    let path = loopback.path().clone();
                    tasks.push(Task::perform(
                        async move {
                            load_signal_from_file(path.clone())
                                .await
                                .map(Arc::new)
                                .map_err(|err| Error::File(path.to_path_buf(), Arc::new(err)))
                        },
                        Message::LoopbackMeasurementLoaded,
                    ));
                }
                for measurement in project.measurements {
                    let path = measurement.path.clone();
                    tasks.push(Task::perform(
                        async move {
                            load_signal_from_file(path.clone())
                                .await
                                .map(Arc::new)
                                .map_err(|err| Error::File(path.to_path_buf(), Arc::new(err)))
                        },
                        Message::MeasurementLoaded,
                    ))
                }

                Task::batch(tasks)
            }
            Message::ProjectLoaded(Err(err)) => {
                dbg!(err);

                Task::none()
            }
            Message::SaveProject => {
                let Raumklang::Loaded {
                    loopback,
                    measurements,
                    ..
                } = self
                else {
                    return Task::none();
                };

                let loopback = loopback.as_ref().map(ProjectLoopback::from);
                let measurements = measurements.iter().map(ProjectMeasurement::from).collect();

                let project = Project {
                    loopback,
                    measurements,
                };

                let content = serde_json::to_string_pretty(&project).unwrap();
                Task::perform(pick_file_and_save(content), Message::ProjectSaved)
            }
            Message::ProjectSaved(Ok(path)) => {
                let Raumklang::Loaded {
                    recent_projects, ..
                } = self
                else {
                    return Task::none();
                };

                recent_projects.insert(path);
                let recent_projects = recent_projects.clone();
                Task::perform(
                    async move { save_recent_projects(&recent_projects).await },
                    |_| {},
                )
                .discard()
            }
            Message::ProjectSaved(Err(err)) => {
                dbg!(err);

                Task::none()
            }
            Message::LoopbackMeasurementLoaded(Ok(new_loopback)) => {
                let Some(new_loopback) = Arc::into_inner(new_loopback) else {
                    return Task::none();
                };

                let Raumklang::Loaded { loopback, .. } = self else {
                    return Task::none();
                };

                *loopback = Some(data::MeasurementState::Loaded(new_loopback));

                Task::none()
            }
            Message::MeasurementLoaded(Ok(measurement)) => {
                let Some(measurement) = Arc::into_inner(measurement) else {
                    return Task::none();
                };

                let Raumklang::Loaded { measurements, .. } = self else {
                    return Task::none();
                };

                measurements.insert(data::MeasurementState::Loaded(measurement));

                Task::none()
            }
            Message::LoopbackMeasurementLoaded(Err(Error::File(path, _err))) => {
                let Raumklang::Loaded { loopback, .. } = self else {
                    return Task::none();
                };

                let measurement = OfflineMeasurement::from_path(path);
                *loopback = Some(data::MeasurementState::NotLoaded(measurement));

                Task::none()
            }
            Message::LoopbackMeasurementLoaded(Err(err)) => {
                dbg!(err);

                Task::none()
            }
            Message::MeasurementLoaded(Err(err)) => {
                match err {
                    Error::File(path, reason) => {
                        let Raumklang::Loaded { measurements, .. } = self else {
                            return Task::none();
                        };

                        let measurement = OfflineMeasurement::from_path(path);
                        measurements.insert(data::MeasurementState::NotLoaded(measurement));

                        dbg!(reason);
                    }
                    Error::DialogClosed => {}
                }

                Task::none()
            }
            Message::TabSelected(tab_id) => {
                let Raumklang::Loaded { active_tab, .. } = self else {
                    return Task::none();
                };

                let cur_tab = mem::take(active_tab);
                *active_tab = match (tab_id, cur_tab) {
                    (TabId::Measurements, _) => Tab::Measurements(tabs::Measurements::default()),
                    (TabId::ImpulseResponse, Tab::Measurements(_)) => {
                        Tab::Analysis(tabs::ImpulseResponseTab::default())
                    }
                    (TabId::ImpulseResponse, tab) => tab,
                };
                Task::none()
            }
            Message::MeasurementsTab(message) => {
                let Raumklang::Loaded {
                    active_tab,
                    loopback,
                    measurements,
                    ..
                } = self
                else {
                    return Task::none();
                };

                let Tab::Measurements(active_tab) = active_tab else {
                    return Task::none();
                };

                let loopback_ref = match &loopback {
                    Some(data::MeasurementState::Loaded(l)) => Some(l),
                    Some(data::MeasurementState::NotLoaded(_)) => None,
                    None => None,
                };
                let measurement_refs: Vec<_> = measurements.loaded().collect();
                let (task, event) = active_tab.update(message, loopback_ref, &measurement_refs);

                let event_task = match event {
                    Some(measurements::Event::LoadLoopbackMeasurement) => Task::perform(
                        pick_file_and_load_signal("loopback"),
                        Message::LoopbackMeasurementLoaded,
                    ),
                    Some(measurements::Event::RemoveLoopbackMeasurement) => {
                        *loopback = None;
                        Task::none()
                    }
                    Some(measurements::Event::LoadMeasurement) => Task::perform(
                        pick_file_and_load_signal("measurement"),
                        Message::MeasurementLoaded,
                    ),
                    Some(measurements::Event::RemoveMeasurement(id)) => {
                        measurements.remove(id);
                        Task::none()
                    }
                    None => Task::none(),
                };

                Task::batch(vec![event_task, task.map(Message::MeasurementsTab)])
            }
            Message::ImpulseResponseTab(message) => {
                let Raumklang::Loaded {
                    active_tab: Tab::Analysis(tab),
                    loopback: Some(data::MeasurementState::Loaded(loopback)),
                    measurements,
                    impulse_responses,
                    ..
                } = self
                else {
                    return Task::none();
                };

                let (task, event) = tab.update(message, loopback, measurements, impulse_responses);

                match event {
                    Some(impulse_response::Event::ImpulseResponseComputed(id, ir)) => {
                        impulse_responses.insert(id, ir);
                    }
                    None => {}
                };

                task.map(Message::ImpulseResponseTab)
            }
            Message::Debug => Task::none(),
            Message::LoadRecentProject(id) => {
                let Raumklang::Loaded {
                    recent_projects, ..
                } = self
                else {
                    return Task::none();
                };

                let Some(project) = recent_projects.get(id) else {
                    return Task::none();
                };

                Task::perform(
                    load_project_from_file(project.clone()),
                    Message::ProjectLoaded,
                )
            }
            Message::RecentProjectsLoaded(Ok(recent_projects)) => {
                let Self::Loading = self else {
                    return Task::none();
                };

                *self = Self::Loaded {
                    loopback: None,
                    measurements: data::Store::new(),
                    impulse_responses: HashMap::new(),
                    recent_projects,
                    active_tab: Tab::default(),
                };

                Task::none()
            }
            Message::RecentProjectsLoaded(Err(_)) => {
                let Self::Loading = self else {
                    return Task::none();
                };

                *self = Self::Loaded {
                    loopback: None,
                    measurements: data::Store::new(),
                    impulse_responses: HashMap::new(),
                    active_tab: Tab::default(),
                    recent_projects: data::RecentProjects::new(MAX_RECENT_PROJECTS_ENTRIES),
                };

                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match self {
            Raumklang::Loading => text("Application is loading").into(),
            Raumklang::Loaded {
                loopback,
                measurements,
                active_tab,
                recent_projects,
                ..
            } => {
                let menu = {
                    let project_menu = {
                        let recent_menu = {
                            let recent_entries: Vec<_> = recent_projects
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
                    let ir_button_msg = match (loopback, measurements.is_loaded_empty()) {
                        (Some(data::MeasurementState::Loaded(_)), false) => {
                            Some(Message::TabSelected(TabId::ImpulseResponse))
                        }
                        _ => None,
                    };

                    fn tab_button(
                        title: &str,
                        active: bool,
                        msg: Option<Message>,
                    ) -> Element<'_, Message> {
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
                            matches!(active_tab, Tab::Measurements(_)),
                            Some(Message::TabSelected(TabId::Measurements))
                        ),
                        tab_button(
                            "Impulse Response",
                            matches!(active_tab, Tab::Analysis(..)),
                            ir_button_msg
                        )
                    ];

                    let tab_content = match &active_tab {
                        Tab::Measurements(tab) => {
                            let measurements: Vec<_> = measurements.iter().collect();
                            tab.view(loopback.as_ref(), measurements)
                                .map(Message::MeasurementsTab)
                        }
                        Tab::Analysis(tab) => {
                            let measurements: Vec<_> = measurements.loaded().collect();
                            tab.view(&measurements).map(Message::ImpulseResponseTab)
                        }
                    };

                    container(column!(tab_bar, tab_content).spacing(5))
                        .width(Length::Fill)
                        .height(Length::Fill)
                };
                let c = column!(menu, content).spacing(8).padding(8);
                //let sc = scrollable(c);
                let back = container(c).width(Length::Fill).height(Length::Fill);

                back.into()
            }
        }
    }
}

pub fn icon<'a, M>(codepoint: char) -> Element<'a, M> {
    const ICON_FONT: Font = Font::with_name("raumklang-icons");

    text(codepoint).font(ICON_FONT).into()
}

pub fn delete_icon<'a, M>() -> Element<'a, M> {
    icon('\u{F1F8}')
}

impl From<&data::MeasurementState<data::Loopback, OfflineMeasurement>> for ProjectLoopback {
    fn from(value: &data::MeasurementState<data::Loopback, OfflineMeasurement>) -> Self {
        let path = match value {
            data::MeasurementState::Loaded(l) => &l.0.path,
            data::MeasurementState::NotLoaded(om) => &om.path,
        };

        ProjectLoopback::new(ProjectMeasurement {
            path: path.to_path_buf(),
        })
    }
}

impl From<data::MeasurementState<&data::Measurement, &OfflineMeasurement>> for ProjectMeasurement {
    fn from(value: data::MeasurementState<&data::Measurement, &OfflineMeasurement>) -> Self {
        let path = match value {
            data::MeasurementState::Loaded(m) => &m.path,
            data::MeasurementState::NotLoaded(m) => &m.path,
        };

        ProjectMeasurement {
            path: path.to_path_buf(),
        }
    }
}

impl OfflineMeasurement {
    fn from_path(path: PathBuf) -> OfflineMeasurement {
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or("Unknown Measurement".to_string());

        Self { name, path }
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

async fn pick_file_and_load() -> Result<(data::Project, PathBuf), PickAndLoadError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Choose project file ...")
        .pick_file()
        .await
        .ok_or(PickAndLoadError::DialogClosed)?;

    load_project_from_file(handle.path()).await
}

async fn load_project_from_file<P: AsRef<Path>>(
    path: P,
) -> Result<(data::Project, PathBuf), PickAndLoadError> {
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

async fn pick_file_and_load_signal<T>(file_type: impl AsRef<str>) -> Result<Arc<T>, Error>
where
    T: FromFile + Send + 'static,
{
    let handle = pick_file(file_type).await?;
    load_signal_from_file(handle.path())
        .await
        .map(Arc::new)
        .map_err(|err| Error::File(handle.path().to_path_buf(), Arc::new(err)))
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

async fn load_signal_from_file<P, T>(path: P) -> Result<T, WavLoadError>
where
    T: FromFile + Send + 'static,
    P: AsRef<Path> + Send + Sync,
{
    let path = path.as_ref().to_owned();
    tokio::task::spawn_blocking(move || T::from_file(path))
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

async fn save_recent_projects(recent_projects: &data::RecentProjects) {
    let app_data_dir = get_app_data_dir();
    tokio::fs::create_dir_all(&app_data_dir).await.unwrap();

    let file_path = get_recent_project_file_path(app_data_dir);
    let contents = serde_json::to_string_pretty(recent_projects).unwrap();
    tokio::fs::write(file_path, contents).await.unwrap();
}

async fn load_recent_projects() -> Result<data::RecentProjects, ()> {
    let app_data_dir = get_app_data_dir();
    let file_path = get_recent_project_file_path(app_data_dir);

    let content = tokio::fs::read(file_path).await.map_err(|_| ())?;
    serde_json::from_slice(&content).map_err(|_| ())
}
