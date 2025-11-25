mod audio;
mod data;
mod icon;
mod log;
mod screen;
mod ui;
mod widget;

use screen::{
    landing,
    main::{self},
    Screen,
};

use data::{project, RecentProjects};

use iced::{futures::FutureExt, Element, Font, Subscription, Task, Theme};

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

const MAX_RECENT_PROJECTS_ENTRIES: usize = 10;

fn main() -> iced::Result {
    match log::init() {
        Ok(_) => (),
        Err(err) => {
            eprintln!("Raumklang: failed to initialize logger: {err}")
        }
    }

    iced::application(Raumklang::new, Raumklang::update, Raumklang::view)
        .title(Raumklang::title)
        .subscription(Raumklang::subscription)
        .theme(Raumklang::theme)
        .font(icon::FONT)
        .default_font(Font::with_name("Noto Sans"))
        .antialiasing(true)
        .run()
}

#[derive(Debug, Clone)]
enum Message {
    RecentProjectsLoaded(Result<data::RecentProjects, data::Error>),
    ProjectLoaded(Result<(Arc<data::Project>, PathBuf), PickAndLoadError>),

    Landing(landing::Message),
    Main(main::Message),
}

struct Raumklang {
    screen: Screen,
    recent_projects: RecentProjects,
}

impl Raumklang {
    fn new() -> (Self, Task<Message>) {
        let app = Self {
            screen: Screen::Loading,
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
            Message::RecentProjectsLoaded(Ok(recent_projects)) => {
                log::debug!("Recent projects loaded: {:?}", recent_projects);

                for path in recent_projects
                    .into_iter()
                    .take(MAX_RECENT_PROJECTS_ENTRIES)
                {
                    self.recent_projects.insert(path);
                }

                self.screen = Screen::Landing;

                Task::none()
            }
            Message::RecentProjectsLoaded(Err(err)) => {
                dbg!(err);

                self.screen = Screen::Landing;

                Task::none()
            }
            Message::Landing(message) => match message {
                landing::Message::New => {
                    self.screen = Screen::Main(screen::Main::default());

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
            Message::Main(message) => {
                let Screen::Main(main_screen) = &mut self.screen else {
                    return Task::none();
                };

                main_screen.update(message).map(Message::Main)
            }
            Message::ProjectLoaded(Ok((project, path))) => match Arc::into_inner(project) {
                Some(project) => {
                    self.recent_projects.insert(path);

                    let (screen, tasks) = screen::Main::from_project(project);
                    self.screen = Screen::Main(screen);

                    Task::batch([
                        tasks.map(Message::Main),
                        Task::future(self.recent_projects.clone().save()).discard(),
                    ])
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
        match &self.screen {
            Screen::Loading => screen::loading(),
            Screen::Landing => screen::landing(&self.recent_projects).map(Message::Landing),
            Screen::Main(main_screen) => main_screen.view().map(Message::Main),
        }
    }

    fn subscription(&self) -> Subscription<Message> {
        match &self.screen {
            Screen::Loading | Screen::Landing => Subscription::none(),
            Screen::Main(main_screen) => main_screen.subscription().map(Message::Main),
        }
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
    File(#[from] project::Error),
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
