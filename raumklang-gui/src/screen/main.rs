pub mod tab;

use std::fmt::Display;

use rustfft::num_complex::Complex;
pub use tab::Tab;
use tab::{frequency_responses, impulse_responses, measurements};

use crate::data::{self};

use iced::{
    widget::{button, center, column, container, horizontal_space, opaque, row, stack, text},
    Alignment, Color, Element, Subscription, Task,
};

#[derive(Default)]
pub struct Main {
    active_tab: Tab,
    project: data::Project,
    pending_window: Option<data::Window<data::Samples>>,
    modal: Modal,
}

#[derive(Default)]
enum Modal {
    #[default]
    None,
    PendingWindow {
        goto_tab: TabId,
    },
    ReplaceLoopback {
        loopback: data::measurement::State<data::measurement::Loopback>,
    },
}

#[derive(Debug, Clone)]
pub enum ModalAction {
    Discard,
    Apply,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(TabId),
    Measurements(measurements::Message),
    ImpulseResponses(impulse_responses::Message),
    FrequencyResponses(frequency_responses::Message),
    ImpulseResponseComputed(Result<(usize, data::ImpulseResponse), data::Error>),
    Modal(ModalAction),
    FrequencyResponseComputed((usize, data::FrequencyResponse)),
    FrequencyResponsesSmoothingComputed((usize, Vec<f32>)),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TabId {
    Measurements,
    ImpulseResponses,
    FrequencyResponses,
}

impl Main {
    pub fn new(project: data::Project) -> Self {
        Self {
            active_tab: Tab::default(),
            pending_window: None,
            modal: Modal::None,
            project,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab_id) => {
                if self.pending_window.is_some() {
                    self.modal = Modal::PendingWindow { goto_tab: tab_id };
                    return Task::none();
                }

                self.goto_tab(tab_id)
            }
            Message::Measurements(message) => {
                let Tab::Measurements(measurements) = &mut self.active_tab else {
                    return Task::none();
                };

                let action = measurements.update(message);

                match action {
                    measurements::Action::LoopbackAdded(loopback) => {
                        if self.project.loopback().is_some() {
                            self.modal = Modal::ReplaceLoopback { loopback };
                        } else {
                            self.project.set_loopback(Some(loopback));
                        }

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
                let Tab::ImpulseResponses(impulse_responses) = &mut self.active_tab else {
                    return Task::none();
                };

                let action = impulse_responses.update(message);

                match action {
                    impulse_responses::Action::ComputeImpulseResponse(id) => {
                        let computation = match self.project.impulse_response_computation(id) {
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
                    impulse_responses::Action::WindowModified(modified) => {
                        if self.project.window() != &modified {
                            self.pending_window = Some(modified);
                        } else {
                            self.pending_window = None;
                        }
                        Task::none()
                    }
                }
            }
            Message::ImpulseResponseComputed(Ok((id, impulse_response))) => {
                self.project
                    .measurements_mut()
                    .get_mut(id)
                    .map(|m| match m {
                        data::measurement::State::NotLoaded(_) => {}
                        data::measurement::State::Loaded(measurement) => {
                            measurement.impulse_response_computed(impulse_response)
                        }
                    });

                Task::none()
            }

            Message::ImpulseResponseComputed(Err(err)) => {
                dbg!(err);
                Task::none()
            }
            Message::Modal(action) => match std::mem::take(&mut self.modal) {
                Modal::None => Task::none(),
                Modal::PendingWindow { goto_tab } => {
                    let Some(pending_window) = self.pending_window.take() else {
                        return Task::none();
                    };

                    match action {
                        ModalAction::Discard => {}
                        ModalAction::Apply => self.project.set_window(pending_window),
                    }

                    self.goto_tab(goto_tab)
                }
                Modal::ReplaceLoopback { loopback } => {
                    match action {
                        ModalAction::Discard => {}
                        ModalAction::Apply => self.project.set_loopback(Some(loopback)),
                    }
                    Task::none()
                }
            },
            Message::FrequencyResponses(message) => {
                let Tab::FrequencyResponses(tab) = &mut self.active_tab else {
                    return Task::none();
                };

                match tab.update(message) {
                    frequency_responses::Action::None => Task::none(),
                    frequency_responses::Action::Smooth(fraction) => {
                        let frequency_responses = self
                            .project
                            .measurements()
                            .iter()
                            .enumerate()
                            .filter_map(|(id, m)| match &m {
                                data::measurement::State::NotLoaded(_details) => None,
                                data::measurement::State::Loaded(measurement) => {
                                    measurement.frequency_response().as_ref().and_then(|fr| {
                                        let data: Vec<f32> = fr
                                            .origin
                                            .data
                                            .iter()
                                            .copied()
                                            .map(|s| s.re.abs())
                                            .collect();

                                        Some((id, data))
                                    })
                                }
                            });

                        let tasks = frequency_responses.map(|(id, data)| {
                            Task::perform(
                                async move {
                                    tokio::task::spawn_blocking(move || {
                                        (
                                            id,
                                            data::frequency_response::nth_octave_smoothing(
                                                &data, fraction,
                                            ),
                                        )
                                    })
                                    .await
                                    .unwrap()
                                },
                                Message::FrequencyResponsesSmoothingComputed,
                            )
                        });

                        Task::batch(tasks)
                    }
                }
            }
            Message::FrequencyResponseComputed((id, frequency_response)) => {
                self.project
                    .measurements_mut()
                    .get_mut(id)
                    .map(|m| match m {
                        data::measurement::State::NotLoaded(_) => {}
                        data::measurement::State::Loaded(measurement) => {
                            measurement.frequency_response_computed(frequency_response)
                        }
                    });

                Task::none()
            }
            Message::FrequencyResponsesSmoothingComputed((id, smoothed)) => {
                let Some(data::measurement::State::Loaded(measurement)) =
                    self.project.measurements_mut().get_mut(id)
                else {
                    return Task::none();
                };

                let Some(frequency_response) = measurement.frequency_response_mut() else {
                    return Task::none();
                };

                frequency_response.smoothed = Some(smoothed.iter().map(Complex::from).collect());

                Task::none()
            }
        }
    }

    fn goto_tab(&mut self, tab_id: TabId) -> Task<Message> {
        let (tab, task) = match tab_id {
            TabId::Measurements => (Tab::Measurements(tab::Measurements::new()), Task::none()),
            TabId::ImpulseResponses => (
                Tab::ImpulseResponses(tab::ImpulseReponses::new(self.project.window())),
                Task::none(),
            ),
            TabId::FrequencyResponses => {
                let ids: Vec<_> = self
                    .project
                    .measurements()
                    .iter()
                    .enumerate()
                    .map(|(id, _)| id)
                    .collect();

                let computations = self.project.all_frequency_response_computations().unwrap();

                let tasks = computations.into_iter().map(|computation| {
                    Task::sip(
                        computation.run(),
                        |res| Message::ImpulseResponseComputed(Ok(res)),
                        Message::FrequencyResponseComputed,
                    )
                });

                let tab = Tab::FrequencyResponses(tab::FrequencyResponses::new(ids.into_iter()));

                (tab, Task::batch(tasks))
            }
        };

        self.active_tab = tab;
        task
    }

    pub fn view(&self) -> Element<Message> {
        let content = {
            let header = { TabId::from(&self.active_tab).view() };

            let content = match &self.active_tab {
                Tab::Measurements(measurements) => {
                    measurements.view(&self.project).map(Message::Measurements)
                }
                Tab::ImpulseResponses(irs) => irs
                    .view(self.project.measurements())
                    .map(Message::ImpulseResponses),
                Tab::FrequencyResponses(frs) => {
                    let loaded: Vec<_> = self
                        .project
                        .measurements()
                        .iter()
                        .filter_map(|state| {
                            if let data::measurement::State::Loaded(measurement) = state {
                                Some(measurement)
                            } else {
                                None
                            }
                        })
                        .collect();

                    frs.view(&loaded).map(Message::FrequencyResponses)
                }
            };

            container(column![header, content].spacing(10))
                .padding(5)
                .style(container::bordered_box)
        };

        match self.modal {
            Modal::None => content.into(),
            Modal::PendingWindow { .. } => {
                let pending_window = {
                    container(
                    column![
                        text("Window pending!").size(18),
                        column![
                            text("You have modified the window used for frequency response computations."),
                            text("You need to discard or apply your changes before proceeding."),
                        ].spacing(5),
                        row![
                            horizontal_space(),
                            button("Discard")
                                .style(button::danger)
                                .on_press(Message::Modal(ModalAction::Discard)),
                            button("Apply")
                                .style(button::success)
                                .on_press(Message::Modal(ModalAction::Apply))
                        ]
                        .spacing(5)
                    ]
                    .spacing(10))
                    .padding(20)
                    .width(400)
                    .style(container::bordered_box)
                };

                modal(content, pending_window).into()
            }
            Modal::ReplaceLoopback { .. } => {
                let pending_window = {
                    container(
                        column![
                            text("Override current Loopback signal!").size(18),
                            column![text(
                                "Do you want to override the current Loopback signal?."
                            ),]
                            .spacing(5),
                            row![
                                horizontal_space(),
                                button("Discard")
                                    .style(button::danger)
                                    .on_press(Message::Modal(ModalAction::Discard)),
                                button("Apply")
                                    .style(button::success)
                                    .on_press(Message::Modal(ModalAction::Apply))
                            ]
                            .spacing(5)
                        ]
                        .spacing(10),
                    )
                    .padding(20)
                    .width(400)
                    .style(container::bordered_box)
                };

                modal(content, pending_window).into()
            }
        }
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match &self.active_tab {
            Tab::Measurements(measurements) => {
                measurements.subscription().map(Message::Measurements)
            }
            Tab::ImpulseResponses(impulse_reponses) => impulse_reponses
                .subscription()
                .map(Message::ImpulseResponses),
            Tab::FrequencyResponses(tab) => tab.subscription().map(Message::FrequencyResponses),
        }
    }
}

impl TabId {
    pub fn iter() -> impl Iterator<Item = Self> {
        [
            TabId::Measurements,
            TabId::ImpulseResponses,
            TabId::FrequencyResponses,
        ]
        .into_iter()
    }

    pub fn view<'a>(self) -> Element<'a, Message> {
        let mut row = row![].spacing(5).align_y(Alignment::Center);

        for tab in TabId::iter() {
            let is_selected = self == tab;

            row = row.push(tab_button(tab, is_selected));
        }

        row.into()
    }
}

impl Display for TabId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            TabId::Measurements => "Measurements",
            TabId::ImpulseResponses => "Impulse Responses",
            TabId::FrequencyResponses => "Frequency Responses",
        };

        write!(f, "{}", label)
    }
}

impl From<&Tab> for TabId {
    fn from(tab: &Tab) -> Self {
        match tab {
            Tab::Measurements(_measurements) => TabId::Measurements,
            Tab::ImpulseResponses(_impulse_reponses) => TabId::ImpulseResponses,
            Tab::FrequencyResponses(_frequency_responses) => TabId::FrequencyResponses,
        }
    }
}

fn tab_button<'a>(tab: TabId, active: bool) -> Element<'a, Message> {
    let style = match active {
        true => button::primary,
        false => button::secondary,
    };

    button(text(tab.to_string()))
        .style(style)
        .on_press(Message::TabSelected(tab))
        .into()
}

fn modal<'a, Message>(
    base: impl Into<Element<'a, Message>>,
    content: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    stack![
        base.into(),
        opaque(center(opaque(content)).style(|_theme| {
            container::Style {
                background: Some(
                    Color {
                        a: 0.8,
                        ..Color::BLACK
                    }
                    .into(),
                ),
                ..container::Style::default()
            }
        }))
    ]
    .into()
}
