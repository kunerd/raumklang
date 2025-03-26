mod data;
mod screen;
// mod widgets;

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use screen::{
    landing,
    main::{self, impulse_responses, measurements, ImpulseReponses, Measurements},
    Screen,
};

use data::{
    project::{file, ImpulseResponseComputation},
    RecentProjects,
};

use iced::{
    futures::FutureExt,
    widget::{button, column, container, row, text, Button},
    Element, Font, Settings, Subscription, Task, Theme,
};

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
    RecentProjectsLoaded(Result<data::RecentProjects, data::Error>),
    // ProjectFileLoaded((data::ProjectFile, PathBuf)),
    Landing(landing::Message),
    ProjectLoaded(Result<(Arc<data::Project>, PathBuf), PickAndLoadError>),
    TabSelected(main::TabId),

    Measurements(measurements::Message),
    ImpulseResponses(impulse_responses::Message),
    ImpulseResponseComputed(Result<(usize, data::ImpulseResponse), data::Error>),
}

struct Raumklang {
    screen: Screen,
    project: data::Project,
    recent_projects: RecentProjects,
}

impl Raumklang {
    fn new() -> (Self, Task<Message>) {
        let app = Self {
            screen: Screen::Loading,
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
                    self.screen = Screen::Main(main::Tab::default());

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
            Message::ProjectLoaded(Ok((project, path))) => match Arc::into_inner(project) {
                Some(project) => {
                    self.project = project;
                    self.recent_projects.insert(path);
                    self.screen = Screen::Main(main::Tab::default());

                    Task::future(self.recent_projects.clone().save()).discard()
                }
                None => Task::none(),
            },
            Message::ProjectLoaded(Err(err)) => {
                dbg!(err);

                Task::none()
            }
            Message::TabSelected(tab_id) => {
                let Screen::Main(tab) = &mut self.screen else {
                    return Task::none();
                };

                *tab = match tab_id {
                    main::TabId::Measurements => main::Tab::Measurements(Measurements::new()),
                    main::TabId::ImpulseResponses => {
                        main::Tab::ImpulseResponses(ImpulseReponses::new())
                    }
                };

                Task::none()
            }
            Message::Measurements(message) => {
                let Screen::Main(main::Tab::Measurements(measurements)) = &mut self.screen else {
                    return Task::none();
                };

                let action = measurements.update(message);

                match action {
                    measurements::Action::LoopbackAdded(loopback) => {
                        self.project.set_loopback(Some(loopback));

                        Task::none()
                    }
                    measurements::Action::RemoveLoopback => {
                        self.project.set_loopback(None);

                        Task::none()
                    }
                    measurements::Action::MeasurementAdded(measurement) => {
                        self.project.push_measurements(measurement);

                        Task::none()
                    }
                    measurements::Action::RemoveMeasurement(id) => {
                        self.project.remove_measurement(id);

                        Task::none()
                    }
                    measurements::Action::Task(task) => task.map(Message::Measurements),
                    measurements::Action::None => Task::none(),
                }
            }
            Message::ImpulseResponses(message) => {
                let Screen::Main(main::Tab::ImpulseResponses(impulse_responses)) = &mut self.screen
                else {
                    return Task::none();
                };

                let action = impulse_responses.update(message);

                match action {
                    impulse_responses::Action::ComputeImpulseResponse(id) => {
                        let computation =
                            match ImpulseResponseComputation::new(id, &mut self.project) {
                                Ok(Some(computation)) => computation,
                                Ok(None) => return Task::none(),
                                Err(err) => {
                                    dbg!(err);
                                    return Task::none();
                                }
                            };

                        Task::perform(computation.run(), Message::ImpulseResponseComputed)
                    }

                    impulse_responses::Action::None => Task::none(),
                }
            }
            Message::ImpulseResponseComputed(Ok((id, impulse_response))) => {
                self.project
                    .measurements_mut()
                    .get_mut(id)
                    .map(|m| match &mut m.state {
                        data::measurement::State::NotLoaded => {}
                        data::measurement::State::Loaded {
                            impulse_response: ir,
                            ..
                        } => {
                            *ir = data::impulse_response::State::Computed(impulse_response);
                        }
                    });

                Task::none()
            }

            Message::ImpulseResponseComputed(Err(err)) => {
                dbg!(err);
                Task::none()
            }
        }
    }

    fn view(&self) -> Element<'_, Message> {
        match &self.screen {
            Screen::Loading => screen::loading(),
            Screen::Landing => screen::landing(&self.recent_projects).map(Message::Landing),
            Screen::Main(tab) => {
                fn tab_button<'a>(
                    text: &'a str,
                    active: bool,
                    msg: Message,
                ) -> Button<'a, Message> {
                    let style = match active {
                        true => button::primary,
                        false => button::secondary,
                    };

                    button(text.as_ref()).style(style).on_press(msg)
                }

                let header = row![
                    tab_button(
                        "Measurements",
                        matches!(tab, main::Tab::Measurements(_)),
                        Message::TabSelected(main::TabId::Measurements)
                    ),
                    tab_button(
                        "Impulse Responses",
                        matches!(tab, main::Tab::ImpulseResponses(_)),
                        Message::TabSelected(main::TabId::ImpulseResponses)
                    )
                ]
                .spacing(5);

                let content = match tab {
                    main::Tab::Measurements(measurements) => {
                        measurements.view(&self.project).map(Message::Measurements)
                    }
                    main::Tab::ImpulseResponses(impulse_responses) => impulse_responses
                        .view(self.project.measurements())
                        .map(Message::ImpulseResponses),
                };

                container(column![header, content].spacing(10))
                    .padding(5)
                    .style(container::bordered_box)
                    .into()
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
