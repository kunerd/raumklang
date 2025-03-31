pub mod tab;

pub use tab::Tab;
use tab::{frequency_responses, impulse_responses, measurements};

use crate::data::{self};

use iced::{
    widget::{
        button, center, column, container, horizontal_space, opaque, row, stack, text, Button,
    },
    Color, Element, Subscription, Task,
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
}

#[derive(Debug, Clone)]
pub enum PendingWindowAction {
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
    PendingWindowModal(PendingWindowAction),
}

#[derive(Debug, Clone)]
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

                self.active_tab = match tab_id {
                    TabId::Measurements => Tab::Measurements(tab::Measurements::new()),
                    TabId::ImpulseResponses => {
                        Tab::ImpulseResponses(tab::ImpulseReponses::new(self.project.window()))
                    }
                    TabId::FrequencyResponses => {
                        Tab::FrequencyResponses(tab::FrequencyResponses::new())
                    }
                };

                Task::none()
            }
            Message::Measurements(message) => {
                let Tab::Measurements(measurements) = &mut self.active_tab else {
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
            Message::PendingWindowModal(action) => {
                let Modal::PendingWindow { goto_tab } =
                    std::mem::replace(&mut self.modal, Modal::None)
                else {
                    return Task::none();
                };

                let Some(pending_window) = self.pending_window.take() else {
                    return Task::none();
                };

                match action {
                    PendingWindowAction::Discard => {}
                    PendingWindowAction::Apply => self.project.set_window(pending_window),
                }

                self.active_tab = match goto_tab {
                    TabId::Measurements => Tab::Measurements(tab::Measurements::new()),
                    TabId::ImpulseResponses => {
                        Tab::ImpulseResponses(tab::ImpulseReponses::new(self.project.window()))
                    }
                    TabId::FrequencyResponses => {
                        Tab::FrequencyResponses(tab::FrequencyResponses::new())
                    }
                };

                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let content = {
            let tab_button = |text, active, msg| -> Button<'_, Message> {
                let style = match active {
                    true => button::primary,
                    false => button::secondary,
                };

                button(text).style(style).on_press(msg)
            };

            let header = row![
                tab_button(
                    "Measurements",
                    matches!(self.active_tab, Tab::Measurements(_)),
                    Message::TabSelected(TabId::Measurements)
                ),
                tab_button(
                    "Impulse Responses",
                    matches!(self.active_tab, Tab::ImpulseResponses(_)),
                    Message::TabSelected(TabId::ImpulseResponses)
                ),
                tab_button(
                    "Frequency Responses",
                    matches!(self.active_tab, Tab::FrequencyResponses(_)),
                    Message::TabSelected(TabId::FrequencyResponses)
                )
            ]
            .spacing(5);

            let content = match &self.active_tab {
                Tab::Measurements(measurements) => {
                    measurements.view(&self.project).map(Message::Measurements)
                }
                Tab::ImpulseResponses(irs) => irs
                    .view(self.project.measurements())
                    .map(Message::ImpulseResponses),
                Tab::FrequencyResponses(frs) => frs.view().map(Message::FrequencyResponses),
            };

            container(column![header, content].spacing(10))
                .padding(5)
                .style(container::bordered_box)
        };

        if let Modal::PendingWindow { .. } = self.modal {
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
                                .on_press(Message::PendingWindowModal(PendingWindowAction::Discard)),
                            button("Apply")
                                .style(button::success)
                                .on_press(Message::PendingWindowModal(PendingWindowAction::Apply))
                        ]
                        .spacing(5)
                    ]
                    .spacing(10))
                    .padding(20)
                    .width(400)
                    .style(container::bordered_box)
            };

            modal(content, pending_window).into()
        } else {
            content.into()
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
            Tab::FrequencyResponses(_frs) => Subscription::none(),
        }
    }
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
