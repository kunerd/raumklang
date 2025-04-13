mod data;
mod log;
mod screen;
mod widgets;

use screen::{
    landing,
    main::{self},
    Screen,
};

use data::{project::file, RecentProjects};

use iced::{futures::FutureExt, widget::text, Element, Font, Settings, Subscription, Task, Theme};

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
                    self.screen = Screen::Main(screen::Main::new(project));

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
