pub mod tab;

use raumklang_core::WavLoadError;
use rfd::FileHandle;

use crate::{
    data::{self},
    icon,
    ui::{self, measurement},
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        button, center, column, container, horizontal_rule, horizontal_space, opaque, row,
        scrollable, stack, text, text::Wrapping, Button,
    },
    Alignment, Color, Element, Length, Subscription, Task, Theme,
};

use std::{fmt::Display, path::PathBuf, sync::Arc};

pub struct Main {
    state: State,
    active_tab: TabId,
    // project: data::Project,
    // impulse_responses: tab::ImpulseReponses,
    // frequency_responses: tab::FrequencyResponses,
    // pending_window: Option<data::Window<data::Samples>>,
    // modal: Modal,
}

enum State {
    CollectingMeasuremnts {
        selected: Option<Selected>,
        loopback: Option<ui::Loopback>,
        measurements: Vec<ui::Measurement>,
    },
    Analysing {
        selected: Option<Selected>,
        window: data::Window<data::Samples>,
        // loopback: Loopback,
        // measurements: Vec<Measurement>,
        // not_loaded: Vec<measurement::State<Measurement>>,
    },
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
    AddMeasurement(MeasurementType),
    SignalLoaded(Arc<LoadedMeasurementType>),
    Select(Selected),
    // Measurements(measurements::Message),
    // ImpulseResponses(impulse_responses::Message),
    // FrequencyResponses(frequency_responses::Message),
    // ImpulseResponseComputed(Result<(measurement::Id, data::ImpulseResponse), data::Error>),
    // Modal(ModalAction),
    // Select(Selected),
}

#[derive(Debug, Clone, Copy)]
pub enum MeasurementType {
    Loopback,
    Normal,
}

#[derive(Debug)]
pub enum LoadedMeasurementType {
    Loopback(ui::Loopback),
    Normal(ui::Measurement),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TabId {
    Measurements,
    ImpulseResponses,
    FrequencyResponses,
}

#[derive(Debug, Clone, Copy)]
pub enum Selected {
    Loopback,
    Measurement(usize),
}

impl Main {
    pub fn new() -> Self {
        Self {
            state: State::CollectingMeasuremnts {
                selected: None,
                loopback: None,
                measurements: vec![],
            },
            active_tab: TabId::Measurements,
        }
        // let state = if !project.has_no_measurements() {
        //     State::CollectingMeasuremnts {
        //         selected: None,
        //         loopback: project.loopback,
        //         measurements: project.measurements.into_iter().collect(),
        //     }
        // } else {
        //     let (loaded, not_loaded): (Vec<measurement::State<_>>, _) = project
        //         .measurements
        //         .into_iter()
        //         .partition(|state| matches!(state, measurement::State::Loaded(..)));

        //     State::Analysing {
        //         selected: None,
        //         window: project.window,
        //         loopback: project
        //             .loopback
        //             .and_then(|s| match s {
        //                 measurement::State::NotLoaded(_) => None,
        //                 measurement::State::Loaded(loopback) => Some(loopback),
        //             })
        //             .unwrap(),
        //         measurements: loaded
        //             .into_iter()
        //             .flat_map(|s| match s {
        //                 measurement::State::NotLoaded(_) => None,
        //                 measurement::State::Loaded(loopback) => Some(loopback),
        //             })
        //             .collect(),
        //         not_loaded,
        //     }
        // };

        // Self {
        //     state,
        //     active_tab: TabId::Measurements,
        // }
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab_id) => {
                self.active_tab = tab_id;
                Task::none()
            }
            Message::AddMeasurement(kind) => {
                let dialog_caption = kind.to_string();

                Task::perform(
                    pick_file_and_load_signal(dialog_caption, kind),
                    Message::SignalLoaded,
                )
            }
            Message::SignalLoaded(result) => {
                match self.state {
                    State::CollectingMeasuremnts {
                        ref mut loopback,
                        ref mut measurements,
                        ..
                    } => match Arc::into_inner(result) {
                        Some(LoadedMeasurementType::Loopback(new_loopback)) => {
                            *loopback = Some(new_loopback)
                        }
                        Some(LoadedMeasurementType::Normal(measurement)) => {
                            measurements.push(measurement)
                        }
                        None => {}
                    },
                    State::Analysing { .. } => todo!(),
                }

                Task::none()
            }
            Message::Select(selected) => todo!(),
        }
    }

    pub fn measurements_tab<'a>(
        &'a self,
        selected: Option<Selected>,
        loopback: Option<&'a ui::Loopback>,
        measurements: &'a [ui::Measurement],
    ) -> Element<'a, Message> {
        let sidebar = {
            let loopback = Category::new("Loopback")
                .push_button(
                    button("+")
                        .on_press_maybe(Some(Message::AddMeasurement(MeasurementType::Loopback)))
                        .style(button::secondary),
                )
                .push_button(
                    button(icon::record())
                        // .on_press(Message::StartRecording(recording::Kind::Loopback))
                        .style(button::secondary),
                )
                .push_entry_maybe(loopback.map(|loopback| loopback_list_entry(selected, loopback)));

            let measurements =
                Category::new("Measurements")
                    .push_button(
                        button("+")
                            .style(button::secondary)
                            .on_press(Message::AddMeasurement(MeasurementType::Normal)),
                    )
                    .push_button(
                        button(icon::record())
                            // .on_press(Message::StartRecording(recording::Kind::Measurement))
                            .style(button::secondary),
                    )
                    .extend_entries(measurements.iter().enumerate().map(|(id, measurement)| {
                        measurement_list_entry(id, selected, measurement)
                    }));

            container(scrollable(
                column![loopback, measurements].spacing(20).padding(10),
            ))
            .style(container::rounded_box)
        };

        let content: Element<_> = 'content: {
            // if let Some(recording) = &self.recording {
            //     break 'content recording.view().map(Message::Recording);
            // }

            let welcome_text = |base_text| -> Element<Message> {
                column![
                    text("Welcome").size(24),
                    column![
                        base_text,
                        row![
                            text("You can load signals from file by pressing [+] or"),
                            button(icon::record()).style(button::secondary) // .on_press(Message::StartRecording(recording::Kind::Measurement))
                        ]
                        .spacing(8)
                        .align_y(Vertical::Center)
                    ]
                    .align_x(Horizontal::Center)
                    .spacing(10)
                ]
                .spacing(16)
                .align_x(Horizontal::Center)
                .into()
            };

            // let content: Element<_> = if project.has_no_measurements() {
            //     welcome_text(text(
            //         "You need to load at least one loopback or measurement signal.",
            //     ))
            // } else {
            //     let signal = self
            //         .selected
            //         .as_ref()
            //         .and_then(|selection| match selection {
            //             Selected::Loopback => project.loopback().and_then(|s| {
            //                 if let data::measurement::State::Loaded(data) = &s {
            //                     Some(data.as_ref().iter())
            //                 } else {
            //                     None
            //                 }
            //             }),
            //             Selected::Measurement(id) => project
            //                 .measurements
            //                 .get(*id)
            //                 .and_then(measurement::State::signal),
            //         });

            //     if let Some(signal) = signal {
            //         Chart::<_, (), _>::new()
            //             .width(Length::Fill)
            //             .height(Length::Fill)
            //             .x_range(self.x_range.clone().unwrap_or(0.0..=signal.len() as f32))
            //             .x_labels(Labels::default().format(&|v| format!("{v:.0}")))
            //             .y_labels(Labels::default().format(&|v| format!("{v:.1}")))
            //             .push_series(
            //                 line_series(signal.copied().enumerate().map(|(i, s)| (i as f32, s)))
            //                     .color(iced::Color::from_rgb8(2, 125, 66)),
            //             )
            //             .on_scroll(|state| {
            //                 let pos = state.get_coords();
            //                 let delta = state.scroll_delta();
            //                 let x_range = state.x_range();
            //                 Message::ChartScroll(pos, delta, x_range)
            //             })
            //             .into()
            //     } else {
            //         welcome_text(text("Select a signal to view its data."))
            //     }
            // };
            //
            let content = welcome_text(text("Select a signal to view its data."));

            container(content).center(Length::Fill).into()
        };

        column!(row![
            container(sidebar).width(Length::FillPortion(1)),
            container(content).width(Length::FillPortion(4))
        ]
        .spacing(8))
        .padding(10)
        .into()
    }

    pub fn view(&self) -> Element<Message> {
        let analysing_enabled = if let State::CollectingMeasuremnts {
            selected,
            loopback,
            measurements,
        } = &self.state
        {
            loopback
                .as_ref()
                .is_some_and(|l| l.inner.loaded().is_some())
                && measurements
                    .iter()
                    .any(|m| matches!(m.inner, measurement::State::Loaded(_)))
        } else {
            false
        };

        let header = container(
            self.active_tab
                // .view(matches!(self.state, State::Analysing { .. })),
                .view(analysing_enabled),
        )
        .style(container::dark);

        let content = match &self.state {
            State::CollectingMeasuremnts {
                selected,
                loopback,
                measurements,
            } => {
                // let measurements: Vec<_> = measurements.iter().map(|s| s.as_ref()).collect();
                self.measurements_tab(
                    *selected,
                    loopback.as_ref(),
                    measurements,
                    // selected.as_ref(),
                )
            }
            State::Analysing {
                selected,
                // loopback,
                // measurements,
                ..
            } => {
                // let loopback = measurement::State::Loaded(loopback));
                // let measurements: Vec<_> = measurements
                //     .iter()
                //     .map(measurement::State::Loaded)
                //     .collect();

                // self.measurements_tab(loopback, measurements, selected.as_ref())
                // self.measurements_tab()
                text("Not implemented, yet!").into()
            }
        };

        container(column![header, content].spacing(10))
            .padding(5)
            .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}

// impl Main {
//     pub fn new(project: data::Project) -> Self {
//         Self {
//             active_tab: Tab::default(),
//             impulse_responses: tab::ImpulseReponses::new(project.window()),
//             frequency_responses: tab::FrequencyResponses::new(),
//             pending_window: None,
//             modal: Modal::None,
//             project,
//         }
//     }

//     pub fn update(&mut self, message: Message) -> Task<Message> {
//         match message {
//             Message::TabSelected(tab_id) => {
//                 if self.pending_window.is_some() {
//                     self.modal = Modal::PendingWindow { goto_tab: tab_id };
//                     return Task::none();
//                 }

//                 self.goto_tab(tab_id)
//             }
//             Message::Measurements(message) => {
//                 let Tab::Measurements(measurements) = &mut self.active_tab else {
//                     return Task::none();
//                 };

//                 let action = measurements.update(message);

//                 match action {
//                     measurements::Action::LoopbackAdded(loopback) => {
//                         if self.project.loopback().is_some() {
//                             self.modal = Modal::ReplaceLoopback { loopback };
//                         } else {
//                             self.project.set_loopback(Some(loopback));
//                         }

//                         Task::none()
//                     }
//                     measurements::Action::RemoveLoopback => {
//                         self.project.set_loopback(None);

//                         Task::none()
//                     }
//                     measurements::Action::MeasurementAdded(measurement) => {
//                         self.project.measurements.push(measurement);

//                         Task::none()
//                     }
//                     measurements::Action::RemoveMeasurement(id) => {
//                         let measurement = self.project.measurements.remove(id);

//                         if let Some(measurement) = measurement.loaded() {
//                             self.frequency_responses.remove(measurement.id);
//                         }

//                         Task::none()
//                     }
//                     measurements::Action::Task(task) => task.map(Message::Measurements),
//                     measurements::Action::None => Task::none(),
//                 }
//             }
//             Message::ImpulseResponses(message) => {
//                 let action = self.impulse_responses.update(message);

//                 match action {
//                     impulse_responses::Action::None => Task::none(),
//                     impulse_responses::Action::ComputeImpulseResponse(id) => {
//                         match self.project.impulse_response_computation(id) {
//                             Ok(Some(computation)) => {
//                                 Task::perform(computation.run(), Message::ImpulseResponseComputed)
//                             }
//                             Ok(None) => Task::none(),
//                             Err(err) => {
//                                 dbg!(err);
//                                 Task::none()
//                             }
//                         }
//                     }

//                     impulse_responses::Action::WindowModified(modified) => {
//                         if self.project.window() != &modified {
//                             self.pending_window = Some(modified);
//                         } else {
//                             self.pending_window = None;
//                         }

//                         Task::none()
//                     }
//                 }
//             }
//             Message::ImpulseResponseComputed(Ok((id, impulse_response))) => {
//                 let entry = self
//                     .project
//                     .impulse_responses
//                     .entry(id)
//                     .or_insert(impulse_response::State::Computing);

//                 *entry = impulse_response::State::Computed(impulse_response.clone());

//                 if let Tab::FrequencyResponses = self.active_tab {
//                     self.frequency_responses
//                         .compute(id, impulse_response, self.project.window().clone())
//                         .map(Message::FrequencyResponses)
//                 } else {
//                     Task::none()
//                 }
//             }

//             Message::ImpulseResponseComputed(Err(err)) => {
//                 dbg!(err);
//                 Task::none()
//             }
//             Message::Modal(action) => match std::mem::take(&mut self.modal) {
//                 Modal::None => Task::none(),
//                 Modal::PendingWindow { goto_tab } => {
//                     let Some(pending_window) = self.pending_window.take() else {
//                         return Task::none();
//                     };

//                     match action {
//                         ModalAction::Discard => {}
//                         ModalAction::Apply => self.project.set_window(pending_window),
//                     }

//                     self.goto_tab(goto_tab)
//                 }
//                 Modal::ReplaceLoopback { loopback } => {
//                     match action {
//                         ModalAction::Discard => {}
//                         ModalAction::Apply => self.project.set_loopback(Some(loopback)),
//                     }
//                     Task::none()
//                 }
//             },
//             Message::FrequencyResponses(message) => {
//                 match self.frequency_responses.update(message, &self.project) {
//                     frequency_responses::Action::None => Task::none(),
//                     frequency_responses::Action::Task(task) => {
//                         task.map(Message::FrequencyResponses)
//                     }
//                     frequency_responses::Action::ImpulseResponseComputed(
//                         id,
//                         impulse_response,
//                         task,
//                     ) => {
//                         let entry = self
//                             .project
//                             .impulse_responses
//                             .entry(id)
//                             .or_insert(impulse_response::State::Computing);

//                         *entry = impulse_response::State::Computed(impulse_response);

//                         task.map(Message::FrequencyResponses)
//                     }
//                 }
//             }
//         }
//     }

//     fn goto_tab(&mut self, tab_id: TabId) -> Task<Message> {
//         let (tab, task) = match tab_id {
//             TabId::Measurements => (Tab::Measurements(tab::Measurements::new()), Task::none()),
//             TabId::ImpulseResponses => (Tab::ImpulseResponses, Task::none()),
//             TabId::FrequencyResponses => {
//                 let loaded_ids: Vec<_> = self.project.measurements.loaded().map(|m| m.id).collect();
//                 let impulse_response_tasks = loaded_ids
//                     .into_iter()
//                     .flat_map(|id| self.project.impulse_response_computation(id).ok())
//                     .flatten()
//                     .map(|computation| {
//                         Task::perform(computation.run(), Message::ImpulseResponseComputed)
//                     });

//                 (Tab::FrequencyResponses, Task::batch(impulse_response_tasks))
//             }
//         };

//         self.active_tab = tab;
//         task
//     }

//     pub fn view(&self) -> Element<Message> {
//         let content = {
//             let header = { TabId::from(&self.active_tab).view() };

//             let content = match &self.active_tab {
//                 Tab::Measurements(measurements) => {
//                     measurements.view(&self.project).map(Message::Measurements)
//                 }
//                 Tab::ImpulseResponses => self
//                     .impulse_responses
//                     .view(&self.project.measurements, &self.project.impulse_responses)
//                     .map(Message::ImpulseResponses),
//                 Tab::FrequencyResponses => self
//                     .frequency_responses
//                     .view(&self.project.measurements)
//                     .map(Message::FrequencyResponses),
//             };

//             container(column![header, content].spacing(10))
//                 .padding(5)
//                 .style(container::bordered_box)
//         };

//         match self.modal {
//             Modal::None => content.into(),
//             Modal::PendingWindow { .. } => {
//                 let pending_window = {
//                     container(
//                     column![
//                         text("Window pending!").size(18),
//                         column![
//                             text("You have modified the window used for frequency response computations."),
//                             text("You need to discard or apply your changes before proceeding."),
//                         ].spacing(5),
//                         row![
//                             horizontal_space(),
//                             button("Discard")
//                                 .style(button::danger)
//                                 .on_press(Message::Modal(ModalAction::Discard)),
//                             button("Apply")
//                                 .style(button::success)
//                                 .on_press(Message::Modal(ModalAction::Apply))
//                         ]
//                         .spacing(5)
//                     ]
//                     .spacing(10))
//                     .padding(20)
//                     .width(400)
//                     .style(container::bordered_box)
//                 };

//                 modal(content, pending_window).into()
//             }
//             Modal::ReplaceLoopback { .. } => {
//                 let pending_window = {
//                     container(
//                         column![
//                             text("Override current Loopback signal!").size(18),
//                             column![text(
//                                 "Do you want to override the current Loopback signal?."
//                             ),]
//                             .spacing(5),
//                             row![
//                                 horizontal_space(),
//                                 button("Discard")
//                                     .style(button::danger)
//                                     .on_press(Message::Modal(ModalAction::Discard)),
//                                 button("Apply")
//                                     .style(button::success)
//                                     .on_press(Message::Modal(ModalAction::Apply))
//                             ]
//                             .spacing(5)
//                         ]
//                         .spacing(10),
//                     )
//                     .padding(20)
//                     .width(400)
//                     .style(container::bordered_box)
//                 };

//                 modal(content, pending_window).into()
//             }
//         }
//     }

//     pub fn subscription(&self) -> Subscription<Message> {
//         match &self.active_tab {
//             Tab::Measurements(measurements) => {
//                 measurements.subscription().map(Message::Measurements)
//             }
//             Tab::ImpulseResponses => self
//                 .impulse_responses
//                 .subscription()
//                 .map(Message::ImpulseResponses),
//             Tab::FrequencyResponses => self
//                 .frequency_responses
//                 .subscription()
//                 .map(Message::FrequencyResponses),
//         }
//     }
// }

impl Default for Main {
    fn default() -> Self {
        Self::new()
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

    pub fn view<'a>(self, is_analysing: bool) -> Element<'a, Message> {
        let mut row = row![].spacing(5).align_y(Alignment::Center);

        for tab in TabId::iter() {
            let is_selected = self == tab;

            let is_enabled = match tab {
                TabId::Measurements => true,
                TabId::ImpulseResponses | TabId::FrequencyResponses => is_analysing,
            };

            row = row.push(tab_button(tab, is_selected, is_enabled));
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

fn tab_button<'a>(tab: TabId, is_active: bool, is_enabled: bool) -> Element<'a, Message> {
    button(text(tab.to_string()))
        .style(move |theme: &Theme, status| {
            if is_active {
                let palette = theme.extended_palette();

                button::Style {
                    background: Some(palette.background.base.color.into()),
                    text_color: palette.background.base.text,
                    ..button::text(theme, status)
                }
            } else {
                button::text(theme, status)
            }
        })
        .on_press_maybe(if is_enabled {
            Some(Message::TabSelected(tab))
        } else {
            None
        })
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

fn loopback_list_entry<'a>(
    selected: Option<Selected>,
    signal: &ui::Loopback,
) -> Element<'a, Message> {
    let info = match &signal.inner {
        measurement::State::NotLoaded => text("Error").style(text::danger),
        measurement::State::Loaded(_inner) => text("TODO: Some info"),
    };

    let content = column![
        column![text("Loopback").size(16)].push(info).spacing(5),
        horizontal_rule(3),
        row![
            horizontal_space(),
            button("...").style(button::secondary),
            button(icon::delete())
                // .on_press(Message::RemoveLoopback)
                .style(button::danger)
        ]
        .spacing(3),
    ]
    .clip(true)
    .spacing(3);

    let style = if let Some(Selected::Loopback) = selected {
        button::primary
    } else {
        button::secondary
    };

    button(content)
        .on_press_maybe(
            signal
                .inner
                .loaded()
                .map(|_| Message::Select(Selected::Loopback)),
        )
        .style(style)
        .width(Length::Fill)
        .into()
}

fn measurement_list_entry<'a>(
    index: usize,
    selected: Option<Selected>,
    signal: &'a ui::Measurement,
) -> Element<'a, Message> {
    let info = match &signal.inner {
        measurement::State::NotLoaded => text("Error").style(text::danger),
        measurement::State::Loaded(_inner) => text("TODO: Some info"),
    };

    let content = column![
        column![text(&signal.name).size(16),].push(info).spacing(5),
        horizontal_rule(3),
        row![
            horizontal_space(),
            button("...").style(button::secondary),
            button(icon::delete())
                // .on_press(Message::RemoveMeasurement(index))
                .style(button::danger)
        ]
        .spacing(3),
    ]
    .clip(true)
    .spacing(3);

    let style = match selected {
        Some(Selected::Measurement(selected)) if selected == index => button::primary,
        _ => button::secondary,
    };

    button(content)
        .on_press_maybe(
            signal
                .inner
                .loaded()
                .map(|_| Message::Select(Selected::Measurement(index))),
        )
        .width(Length::Fill)
        .style(style)
        .into()
}

// async fn pick_file_and_load_signal<T>(file_type: impl AsRef<str>) -> Result<Arc<T>, Error>
// where
//     T: FromFile + Send + 'static,
// {
//     let handle = pick_file(file_type).await?;
//     measurement::load_from_file(handle.path())
//         .await
//         .map(Arc::new)
//         .map_err(|err| Error::File(handle.path().to_path_buf(), Arc::new(err)))
// }

// async fn pick_file(file_type: impl AsRef<str>) -> Result<FileHandle, Error> {
//     rfd::AsyncFileDialog::new()
//         .set_title(format!("Choose {} file", file_type.as_ref()))
//         .add_filter("wav", &["wav", "wave"])
//         .add_filter("all", &["*"])
//         .pick_file()
//         .await
//         .ok_or(Error::DialogClosed)
// }

pub struct Category<'a, Message> {
    title: &'a str,
    entries: Vec<Element<'a, Message>>,
    buttons: Vec<Button<'a, Message>>,
}

impl<'a, Message> Category<'a, Message>
where
    Message: 'a + Clone,
{
    pub fn new(title: &'a str) -> Self {
        Self {
            title,
            entries: vec![],
            buttons: vec![],
        }
    }

    pub fn push_button(mut self, button: Button<'a, Message>) -> Self {
        self.buttons.push(button);
        self
    }

    pub fn push_entry(mut self, entry: impl Into<Element<'a, Message>>) -> Self {
        self.entries.push(entry.into());
        self
    }

    pub fn push_entry_maybe(self, entry: Option<impl Into<Element<'a, Message>>>) -> Self {
        if let Some(entry) = entry {
            self.push_entry(entry)
        } else {
            self
        }
    }

    pub fn extend_entries(self, entries: impl IntoIterator<Item = Element<'a, Message>>) -> Self {
        entries.into_iter().fold(self, Self::push_entry)
    }

    pub fn view(self) -> Element<'a, Message> {
        let header = row![container(text(self.title).wrapping(Wrapping::WordOrGlyph))
            .width(Length::Fill)
            .clip(true),]
        .extend(self.buttons.into_iter().map(|btn| btn.width(30).into()))
        .spacing(5)
        .padding(5)
        .align_y(Alignment::Center);

        column!(header, horizontal_rule(1))
            .extend(self.entries.into_iter())
            .width(Length::Fill)
            .spacing(5)
            .into()
    }
}

impl<'a, Message> From<Category<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone,
{
    fn from(category: Category<'a, Message>) -> Self {
        category.view()
    }
}

impl Display for MeasurementType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            MeasurementType::Loopback => "Loopback",
            MeasurementType::Normal => "Measurement",
        };

        write!(f, "{}", text)
    }
}

async fn pick_file_and_load_signal(
    file_type: impl AsRef<str>,
    kind: MeasurementType,
) -> Arc<LoadedMeasurementType> {
    let handle = pick_file(file_type).await.unwrap();

    let path = handle.path();

    let measurement = match kind {
        MeasurementType::Loopback => {
            LoadedMeasurementType::Loopback(ui::Loopback::from_file(path).await)
        }
        MeasurementType::Normal => {
            LoadedMeasurementType::Normal(ui::Measurement::from_file(path).await)
        }
    };

    Arc::new(measurement)
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

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    #[error("error while loading file: {0}")]
    File(PathBuf, Arc<WavLoadError>),
    #[error("dialog closed")]
    DialogClosed,
}
