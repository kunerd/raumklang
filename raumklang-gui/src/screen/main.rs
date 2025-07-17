mod measurement;

use raumklang_core::WavLoadError;
use rfd::FileHandle;
use tracing::Instrument;

use crate::{
    data::{self},
    icon,
    ui::{self},
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        button, center, column, container, horizontal_rule, horizontal_space, opaque, row,
        scrollable, stack, text, text::Wrapping, Button,
    },
    Alignment, Color, Element, Length, Subscription, Task, Theme,
};

use std::{collections::HashMap, fmt::Display, path::PathBuf, sync::Arc};

pub struct Main {
    state: State,
    selected: Option<measurement::Selected>,
    loopback: Option<ui::Loopback>,
    measurements: Vec<ui::Measurement>,
    // project: data::Project,
    // impulse_responses: tab::ImpulseReponses,
    // frequency_responses: tab::FrequencyResponses,
    // pending_window: Option<data::Window<data::Samples>>,
    // modal: Modal,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    CollectingMeasuremnts,
    Analysing {
        active_tab: Tab,
        impulse_responses: HashMap<ui::measurement::Id, raumklang_core::ImpulseResponse>,
    },
}

#[derive(Default)]
enum Modal {
    #[default]
    None,
    PendingWindow {
        goto_tab: Tab,
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
    TabSelected(Tab),
    Measurements(measurement::Message),
    // Measurements(measurements::Message),
    // ImpulseResponses(impulse_responses::Message),
    // FrequencyResponses(frequency_responses::Message),
    // ImpulseResponseComputed(Result<(measurement::Id, data::ImpulseResponse), data::Error>),
    // Modal(ModalAction),
    // Select(Selected),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Measurements,
    ImpulseResponses,
    FrequencyResponses,
}

impl Main {
    pub fn new() -> Self {
        Self {
            state: State::CollectingMeasuremnts,
            selected: None,
            loopback: None,
            measurements: vec![],
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
            Message::TabSelected(tab) => {
                let State::Analysing { active_tab, .. } = &mut self.state else {
                    return Task::none();
                };

                *active_tab = tab;

                Task::none()
            }
            Message::Measurements(message) => {
                let task = match message {
                    measurement::Message::Load(kind) => {
                        let dialog_caption = kind.to_string();

                        Task::perform(
                            measurement::pick_file_and_load_signal(dialog_caption, kind),
                            measurement::Message::Loaded,
                        )
                    }
                    measurement::Message::Loaded(result) => {
                        match Arc::into_inner(result) {
                            Some(measurement::LoadedKind::Loopback(loopback)) => {
                                self.loopback = Some(loopback)
                            }
                            Some(measurement::LoadedKind::Normal(measurement)) => {
                                self.measurements.push(measurement)
                            }
                            None => {}
                        }

                        Task::none()
                    }
                    measurement::Message::Remove(index) => {
                        self.measurements.remove(index);
                        Task::none()
                    }
                    measurement::Message::Select(selected) => todo!(),
                };

                let state = std::mem::take(&mut self.state);
                self.state = match (state, self.analysing_possible()) {
                    (State::CollectingMeasuremnts, false) => State::CollectingMeasuremnts,
                    (State::CollectingMeasuremnts, true) => State::Analysing {
                        active_tab: Tab::Measurements,
                        impulse_responses: HashMap::new(),
                    },
                    (
                        State::Analysing {
                            active_tab,
                            impulse_responses,
                        },
                        true,
                    ) => State::Analysing {
                        active_tab,
                        impulse_responses,
                    },
                    (State::Analysing { .. }, false) => State::CollectingMeasuremnts,
                };

                task.map(Message::Measurements)
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let header = container(match &self.state {
            State::CollectingMeasuremnts => Tab::Measurements.view(false),
            State::Analysing { active_tab, .. } => active_tab.view(true),
        })
        .style(container::dark);

        let content = match self.state {
            State::CollectingMeasuremnts => self.measurements_tab().map(Message::Measurements),
            State::Analysing { active_tab, .. } => match active_tab {
                Tab::Measurements => self.measurements_tab().map(Message::Measurements),
                Tab::ImpulseResponses => self.impulse_responses_tab(),
                Tab::FrequencyResponses => self.frequency_responses_tab(),
            },
        };

        container(column![header, content].spacing(10))
            .padding(5)
            .into()
    }

    fn measurements_tab(&self) -> Element<'_, measurement::Message> {
        let sidebar = {
            let loopback = Category::new("Loopback")
                .push_button(
                    button("+")
                        .on_press_maybe(Some(measurement::Message::Load(
                            measurement::Kind::Loopback,
                        )))
                        .style(button::secondary),
                )
                .push_button(
                    button(icon::record())
                        // .on_press(Message::StartRecording(recording::Kind::Loopback))
                        .style(button::secondary),
                )
                .push_entry_maybe(
                    self.loopback
                        .as_ref()
                        .map(|loopback| measurement::loopback_entry(self.selected, loopback)),
                );

            let measurements =
                Category::new("Measurements")
                    .push_button(
                        button("+")
                            .style(button::secondary)
                            .on_press(measurement::Message::Load(measurement::Kind::Normal)),
                    )
                    .push_button(
                        button(icon::record())
                            // .on_press(Message::StartRecording(recording::Kind::Measurement))
                            .style(button::secondary),
                    )
                    .extend_entries(self.measurements.iter().enumerate().map(
                        |(id, measurement)| measurement::list_entry(id, self.selected, measurement),
                    ));

            container(scrollable(
                column![loopback, measurements].spacing(20).padding(10),
            ))
            .style(container::rounded_box)
        };

        let content: Element<_> = 'content: {
            // if let Some(recording) = &self.recording {
            //     break 'content recording.view().map(Message::Recording);
            // }

            let welcome_text = |base_text| -> Element<measurement::Message> {
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

    fn impulse_responses_tab(&self) -> Element<'_, Message> {
        text("Not implemented, yet!").into()
        // let sidebar = {
        //     let header = {
        //         column!(text("For Measurements"), horizontal_rule(1))
        //             .width(Length::Fill)
        //             .spacing(5)
        //     };

        //     let entries = measurements
        //         .loaded()
        //         .map(|measurement| (measurement, impulse_responses.get(&measurement.id)))
        //         .map(|(measurement, ir)| {
        //             let id = measurement.id;

        //             let entry = {
        //                 let content = column![text(&measurement.details.name).size(16),]
        //                     .spacing(5)
        //                     .clip(true)
        //                     .spacing(3);

        //                 let style = match self.selected.as_ref() {
        //                     Some(selected) if *selected == id => button::primary,
        //                     _ => button::secondary,
        //                 };

        //                 button(content)
        //                     .on_press_with(move || Message::Select(id))
        //                     .width(Length::Fill)
        //                     .style(style)
        //                     .into()
        //             };

        //             if let Some(ir) = ir {
        //                 match &ir {
        //                     impulse_response::State::Computing => {
        //                         processing_overlay("Impulse Response", entry)
        //                     }
        //                     impulse_response::State::Computed(_) => entry,
        //                 }
        //             } else {
        //                 entry
        //             }
        //         });

        //     container(scrollable(
        //         column![header, column(entries).spacing(3)]
        //             .spacing(10)
        //             .padding(10),
        //     ))
        //     .style(container::rounded_box)
        // }
        // .width(Length::FillPortion(1));

        // let content: Element<_> = {
        //     if let Some(id) = self.selected {
        //         let state = impulse_responses.get(&id).and_then(|ir| match &ir {
        //             impulse_response::State::Computing => None,
        //             impulse_response::State::Computed(ir) => Some(ir),
        //         });

        //         match state {
        //             Some(impulse_response) => {
        //                 let header = row![pick_list(
        //                     &chart::AmplitudeUnit::ALL[..],
        //                     Some(&self.chart_data.amplitude_unit),
        //                     |unit| Message::Chart(ChartOperation::AmplitudeUnitChanged(unit))
        //                 ),]
        //                 .align_y(Alignment::Center)
        //                 .spacing(10);

        //                 let chart = {
        //                     let x_scale_fn = match self.chart_data.time_unit {
        //                         chart::TimeSeriesUnit::Samples => sample_scale,
        //                         chart::TimeSeriesUnit::Time => time_scale,
        //                     };

        //                     let y_scale_fn: fn(f32, f32) -> f32 =
        //                         match self.chart_data.amplitude_unit {
        //                             chart::AmplitudeUnit::PercentFullScale => percent_full_scale,
        //                             chart::AmplitudeUnit::DezibelFullScale => db_full_scale,
        //                         };

        //                     let sample_rate = impulse_response.sample_rate as f32;

        //                     let chart = Chart::new()
        //                         .width(Length::Fill)
        //                         .height(Length::Fill)
        //                         .cache(&self.chart_data.cache)
        //                         .x_range(
        //                             self.chart_data
        //                                 .x_range
        //                                 .as_ref()
        //                                 .map(|r| {
        //                                     x_scale_fn(*r.start(), sample_rate)
        //                                         ..=x_scale_fn(*r.end(), sample_rate)
        //                                 })
        //                                 .unwrap_or_else(|| {
        //                                     x_scale_fn(-sample_rate / 2.0, sample_rate)
        //                                         ..=x_scale_fn(
        //                                             impulse_response.data.len() as f32,
        //                                             sample_rate,
        //                                         )
        //                                 }),
        //                         )
        //                         .x_labels(Labels::default().format(&|v| format!("{v:.2}")))
        //                         .y_labels(Labels::default().format(&|v| format!("{v:.2}")))
        //                         .push_series(
        //                             line_series(impulse_response.data.iter().enumerate().map(
        //                                 move |(i, s)| {
        //                                     (
        //                                         x_scale_fn(i as f32, sample_rate),
        //                                         y_scale_fn(*s, impulse_response.max),
        //                                     )
        //                                 },
        //                             ))
        //                             .color(iced::Color::from_rgb8(2, 125, 66)),
        //                         )
        //                         .on_scroll(|state| {
        //                             let pos = state.get_coords();
        //                             let delta = state.scroll_delta();
        //                             let x_range = state.x_range();
        //                             Message::Chart(ChartOperation::Scroll(pos, delta, x_range))
        //                         });

        //                     let window_curve = self.window_settings.window.curve();
        //                     let handles: window::Handles = Into::into(&self.window_settings.window);
        //                     chart
        //                         .push_series(
        //                             line_series(window_curve.map(move |(i, s)| {
        //                                 (x_scale_fn(i, sample_rate), y_scale_fn(s, 1.0))
        //                             }))
        //                             .color(iced::Color::from_rgb8(255, 0, 0)),
        //                         )
        //                         .push_series(
        //                             point_series(handles.into_iter().map(move |handle| {
        //                                 (
        //                                     x_scale_fn(handle.x(), sample_rate),
        //                                     y_scale_fn(handle.y().into(), 1.0),
        //                                 )
        //                             }))
        //                             .with_id(SeriesId::Handles)
        //                             .style_for_each(|index, _handle| {
        //                                 if self.window_settings.hovered.is_some_and(|i| i == index)
        //                                 {
        //                                     point::Style {
        //                                         color: Some(iced::Color::from_rgb8(220, 250, 250)),
        //                                         radius: 10.0,
        //                                         ..Default::default()
        //                                     }
        //                                 } else {
        //                                     point::Style::default()
        //                                 }
        //                             })
        //                             .color(iced::Color::from_rgb8(255, 0, 0)),
        //                         )
        //                         .on_press(|state| {
        //                             let id = state.items().and_then(|l| l.first().map(|i| i.1));
        //                             Message::Window(WindowOperation::MouseDown(
        //                                 id,
        //                                 state.get_offset(),
        //                             ))
        //                         })
        //                         .on_move(|state| {
        //                             let id = state.items().and_then(|l| l.first().map(|i| i.1));
        //                             Message::Window(WindowOperation::OnMove(id, state.get_offset()))
        //                         })
        //                         .on_release(|state| {
        //                             Message::Window(WindowOperation::MouseUp(state.get_offset()))
        //                         })
        //                 };

        //                 let footer = {
        //                     row![
        //                         horizontal_space(),
        //                         pick_list(
        //                             &chart::TimeSeriesUnit::ALL[..],
        //                             Some(&self.chart_data.time_unit),
        //                             |unit| {
        //                                 Message::Chart(ChartOperation::TimeUnitChanged(unit))
        //                             }
        //                         ),
        //                     ]
        //                     .align_y(Alignment::Center)
        //                 };

        //                 container(column![header, chart, footer]).into()
        //             }
        //             // TODO: add spinner
        //             None => text("Impulse response not computed, yet.").into(),
        //         }
        //     } else {
        //         text("Please select an entry to view its data.").into()
        //     }
        // };

        // row![
        //     container(sidebar).width(Length::FillPortion(1)),
        //     container(content).center(Length::FillPortion(4))
        // ]
        // .spacing(10)
        // .into()
    }

    fn frequency_responses_tab(&self) -> Element<'_, Message> {
        text("Not implemented, yet!").into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }

    fn analysing_possible(&self) -> bool {
        self.loopback.as_ref().is_some_and(ui::Loopback::is_loaded)
            && self.measurements.iter().any(ui::Measurement::is_loaded)
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

impl Tab {
    pub fn iter() -> impl Iterator<Item = Self> {
        [
            Tab::Measurements,
            Tab::ImpulseResponses,
            Tab::FrequencyResponses,
        ]
        .into_iter()
    }

    pub fn view<'a>(self, is_analysing: bool) -> Element<'a, Message> {
        let mut row = row![].spacing(5).align_y(Alignment::Center);

        for tab in Tab::iter() {
            let is_active = self == tab;

            let is_enabled = match tab {
                Tab::Measurements => true,
                Tab::ImpulseResponses | Tab::FrequencyResponses => is_analysing,
            };
            let button = button(text(tab.to_string()))
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
                });

            row = row.push(button);
        }

        row.into()
    }
}

impl Display for Tab {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let label = match self {
            Tab::Measurements => "Measurements",
            Tab::ImpulseResponses => "Impulse Responses",
            Tab::FrequencyResponses => "Frequency Responses",
        };

        write!(f, "{}", label)
    }
}

fn tab_button<'a>(tab: Tab, is_active: bool, is_enabled: bool) -> Element<'a, Message> {
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
