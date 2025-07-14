pub mod tab;

pub use tab::Tab;

use crate::data::{self, measurement};
use tab::{frequency_responses, impulse_responses, measurements};

use iced::{
    widget::{button, center, column, container, horizontal_space, opaque, row, stack, text},
    Alignment, Color, Element, Subscription, Task,
};

use std::fmt::Display;

pub struct Main {
    active_tab: Tab,
    project: data::Project,
    impulse_responses: tab::ImpulseReponses,
    frequency_responses: tab::FrequencyResponses,
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
    ImpulseResponseComputed(Result<(measurement::Id, data::ImpulseResponse), data::Error>),
    Modal(ModalAction),
    // FrequencyResponseComputed((usize, data::FrequencyResponse)),
    // FrequencyResponsesSmoothingComputed((usize, Vec<f32>)),
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
            impulse_responses: tab::ImpulseReponses::new(project.window()),
            frequency_responses: tab::FrequencyResponses::new(),
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
                        self.project.measurements.push(measurement);

                        Task::none()
                    }
                    measurements::Action::RemoveMeasurement(id) => {
                        let measurement = self.project.measurements.remove(id);

                        if let Some(measurement) = measurement.loaded() {
                            self.impulse_responses.remove(measurement.id);
                        }

                        Task::none()
                    }
                    measurements::Action::Task(task) => task.map(Message::Measurements),
                    measurements::Action::None => Task::none(),
                }
            }
            Message::ImpulseResponses(message) => {
                let action = self.impulse_responses.update(message);

                match action {
                    impulse_responses::Action::None => Task::none(),
                    impulse_responses::Action::ComputeImpulseResponse(id) => {
                        let computation = match self.project.impulse_response_computation(id) {
                            Ok(computation) => computation,
                            Err(err) => {
                                dbg!(err);
                                return Task::none();
                            }
                        };

                        Task::perform(computation.run(), Message::ImpulseResponseComputed)
                    }

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
                self.impulse_responses.computed(id, impulse_response);

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
                match self.frequency_responses.update(message) {
                    frequency_responses::Action::None => Task::none(),
                    frequency_responses::Action::Smooth(fraction) => {
                        // let Some(fraction) = fraction else {
                        //     self.project
                        //         .measurements
                        //         .loaded_mut()
                        //         .flat_map(|m| m.frequency_response_mut())
                        //         .for_each(|fr| fr.smoothed = None);

                        //     return Task::none();
                        // };

                        // let frequency_responses = self
                        //     .project
                        //     .measurements
                        //     .iter()
                        //     .enumerate()
                        //     .filter_map(|(id, m)| match &m {
                        //         data::measurement::State::NotLoaded(_details) => None,
                        //         data::measurement::State::Loaded(measurement) => {
                        //             measurement.frequency_response().as_ref().and_then(|fr| {
                        //                 let data: Vec<f32> = fr
                        //                     .origin
                        //                     .data
                        //                     .iter()
                        //                     .copied()
                        //                     .map(|s| s.re.abs())
                        //                     .collect();

                        //                 Some((id, data))
                        //             })
                        //         }
                        //     });

                        // let tasks = frequency_responses.map(|(id, data)| {
                        //     Task::perform(
                        //         async move {
                        //             tokio::task::spawn_blocking(move || {
                        //                 (
                        //                     id,
                        //                     data::frequency_response::smooth_fractional_octave(
                        //                         &data, fraction,
                        //                     ),
                        //                 )
                        //             })
                        //             .await
                        //             .unwrap()
                        //         },
                        //         Message::FrequencyResponsesSmoothingComputed,
                        //     )
                        // });

                        // Task::batch(tasks)
                        Task::none()
                    }
                }
            } // Message::FrequencyResponseComputed((id, frequency_response)) => {
              //     self.project
              //         .measurements
              //         .get_loaded_mut(id)
              //         .map(|measurement| measurement.frequency_response_computed(frequency_response));

              //     if let Tab::FrequencyResponses(ref tab) = self.active_tab {
              //         tab.clear_cache();
              //     }

              //     Task::none()
              // }
              // Message::FrequencyResponsesSmoothingComputed((id, smoothed)) => {
              //     let Some(measurement) = self.project.measurements.get_loaded_mut(id) else {
              //         return Task::none();
              //     };

              //     let Some(frequency_response) = measurement.frequency_response_mut() else {
              //         return Task::none();
              //     };

              //     frequency_response.smoothed = Some(smoothed.iter().map(Complex::from).collect());

              //     if let Tab::FrequencyResponses(ref tab) = self.active_tab {
              //         tab.clear_cache();
              //     }

              //     Task::none()
              // }
        }
    }

    fn goto_tab(&mut self, tab_id: TabId) -> Task<Message> {
        let (tab, task) = match tab_id {
            TabId::Measurements => (Tab::Measurements(tab::Measurements::new()), Task::none()),
            TabId::ImpulseResponses => (Tab::ImpulseResponses, Task::none()),
            TabId::FrequencyResponses => (Tab::FrequencyResponses, Task::none()),
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
                Tab::ImpulseResponses => self
                    .impulse_responses
                    .view(&self.project.measurements)
                    .map(Message::ImpulseResponses),
                Tab::FrequencyResponses => self
                    .frequency_responses
                    .view(&self.project.measurements)
                    .map(Message::FrequencyResponses),
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
            Tab::ImpulseResponses => self
                .impulse_responses
                .subscription()
                .map(Message::ImpulseResponses),
            Tab::FrequencyResponses => self
                .frequency_responses
                .subscription()
                .map(Message::FrequencyResponses),
        }
    }
}

impl Default for Main {
    fn default() -> Self {
        Self::new(data::Project::default())
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
            Tab::ImpulseResponses => TabId::ImpulseResponses,
            Tab::FrequencyResponses => TabId::FrequencyResponses,
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
