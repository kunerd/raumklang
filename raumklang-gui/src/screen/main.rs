pub mod tab;

pub use tab::Tab;
use tab::{impulse_responses, measurements};

use crate::data::{self, project};

use iced::{
    widget::{button, column, container, row, Button},
    Element, Subscription, Task,
};

#[derive(Default)]
pub struct Main {
    active_tab: Tab,
    project: data::Project,
}

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(TabId),
    Measurements(measurements::Message),
    ImpulseResponses(impulse_responses::Message),
    ImpulseResponseComputed(Result<(usize, data::ImpulseResponse), data::Error>),
}

#[derive(Debug, Clone)]
pub enum TabId {
    Measurements,
    ImpulseResponses,
}
impl Main {
    pub fn new(project: data::Project) -> Self {
        Self {
            active_tab: Tab::default(),
            project,
        }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab_id) => {
                self.active_tab = match tab_id {
                    TabId::Measurements => Tab::Measurements(tab::Measurements::new()),
                    TabId::ImpulseResponses => {
                        Tab::ImpulseResponses(tab::ImpulseReponses::new(self.project.window()))
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
                        let computation =
                            match project::ImpulseResponseComputation::new(id, &mut self.project) {
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

    pub fn view(&self) -> Element<Message> {
        fn tab_button<'a>(text: &'a str, active: bool, msg: Message) -> Button<'a, Message> {
            let style = match active {
                true => button::primary,
                false => button::secondary,
            };

            button(text.as_ref()).style(style).on_press(msg)
        }

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
            )
        ]
        .spacing(5);

        let content = match &self.active_tab {
            Tab::Measurements(measurements) => {
                measurements.view(&self.project).map(Message::Measurements)
            }
            Tab::ImpulseResponses(impulse_responses) => impulse_responses
                .view(self.project.measurements())
                .map(Message::ImpulseResponses),
        };

        container(column![header, content].spacing(10))
            .padding(5)
            .style(container::bordered_box)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        match &self.active_tab {
            Tab::Measurements(measurements) => {
                measurements.subscription().map(Message::Measurements)
            }
            Tab::ImpulseResponses(impulse_reponses) => impulse_reponses
                .subscription()
                .map(Message::ImpulseResponses),
        }
    }
}
