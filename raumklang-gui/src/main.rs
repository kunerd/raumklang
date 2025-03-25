mod data;
mod tab;
// mod widgets;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use tab::{landing, measurements, Measurements, Tab};

use data::{project::file, RecentProjects};

use iced::{futures::FutureExt, widget::text, Element, Font, Settings, Subscription, Task, Theme};

const MAX_RECENT_PROJECTS_ENTRIES: usize = 10;

fn main() -> iced::Result {
    iced::application(Raumklang::title, Raumklang::update, Raumklang::view)
        .subscription(Raumklang::subscription)
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

#[derive(Debug, Clone)]
enum Message {
    RecentProjectsLoaded(data::RecentProjects),
    // ProjectFileLoaded((data::ProjectFile, PathBuf)),
    Landing(landing::Message),
    Measurements(measurements::Message),
    ProjectLoaded(Result<(Arc<data::Project>, PathBuf), PickAndLoadError>),
}

struct Raumklang {
    tab: Tab,
    project: data::Project,
    recent_projects: RecentProjects,
}

impl Raumklang {
    fn new() -> (Self, Task<Message>) {
        let app = Self {
            tab: Tab::Loading,
            project: data::Project::new(),
            recent_projects: RecentProjects::new(MAX_RECENT_PROJECTS_ENTRIES),
        };
        let task = Task::perform(RecentProjects::load(), Message::RecentProjectsLoaded);

        (app, task)
    }

    fn title(&self) -> String {
        "Raumklang".to_string()
    }

    fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::RecentProjectsLoaded(recent_projects) => {
                for path in recent_projects
                    .into_iter()
                    .take(MAX_RECENT_PROJECTS_ENTRIES)
                {
                    self.recent_projects.insert(path);
                }

                self.tab = Tab::Landing;

                Task::none()
            }
            Message::Landing(message) => match message {
                landing::Message::New => {
                    self.tab = Tab::Measurements(tab::Measurements::new());

                    Task::none()
                }
                landing::Message::Load => Task::perform(
                    pick_project_file().then(async |res| {
                        let path = res?;
                        load_project(path).await
                    }),
                    Message::ProjectLoaded,
                ),
                landing::Message::Recent(id) => match self.recent_projects.get(id) {
                    Some(path) => Task::perform(load_project(path.clone()), Message::ProjectLoaded),
                    None => Task::none(),
                },
            },
            Message::Measurements(message) => {
                let Tab::Measurements(measurements) = &mut self.tab else {
                    return Task::none();
                };

                let action = measurements.update(message);

                match action {
                    measurements::Action::LoopbackAdded(loopback) => {
                        self.project.loopback = Some(loopback);

                        Task::none()
                    }
                    measurements::Action::RemoveLoopback => {
                        self.project.loopback = None;

                        Task::none()
                    }
                    measurements::Action::MeasurementAdded(measurement) => {
                        self.project.measurements.push(measurement);

                        Task::none()
                    }
                    measurements::Action::RemoveMeasurement(id) => {
                        self.project.measurements.remove(id);

                        Task::none()
                    }
                    measurements::Action::Task(task) => task.map(Message::Measurements),
                    measurements::Action::None => Task::none(),
                }
            }
            Message::ProjectLoaded(Ok((project, path))) => match Arc::into_inner(project) {
                Some(project) => {
                    self.project = project;
                    self.recent_projects.insert(path);
                    self.tab = Tab::Measurements(Measurements::new());

                    Task::future(self.recent_projects.clone().save()).discard()
                }
                None => Task::none(),
            },
            Message::ProjectLoaded(Err(err)) => {
                dbg!(err);

                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.tab {
            Tab::Loading => tab::loading(),
            Tab::Landing => tab::landing(&self.recent_projects).map(Message::Landing),
            Tab::Measurements(measurements) => {
                measurements.view(&self.project).map(Message::Measurements)
            }
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn theme(&self) -> Theme {
        Theme::TokyoNight
    }
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum PickAndLoadError {
    #[error("dialog closed")]
    DialogClosed,
    #[error(transparent)]
    File(#[from] file::Error),
}

pub fn icon<'a, M>(codepoint: char) -> Element<'a, M> {
    const ICON_FONT: Font = Font::with_name("raumklang-icons");

    text(codepoint).font(ICON_FONT).into()
}

pub fn delete_icon<'a, M>() -> Element<'a, M> {
    icon('\u{F1F8}')
}

async fn pick_project_file() -> Result<PathBuf, PickAndLoadError> {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Choose project file ...")
        .pick_file()
        .await
        .ok_or(PickAndLoadError::DialogClosed)?;

    Ok(handle.path().to_path_buf())
}

async fn load_project(
    path: impl AsRef<Path>,
) -> Result<(Arc<data::Project>, PathBuf), PickAndLoadError> {
    let path = path.as_ref();
    let project = data::Project::load(path).await.map(Arc::new)?;

    Ok((project, path.to_path_buf()))
}
