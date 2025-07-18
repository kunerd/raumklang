mod frequency_response;
mod impulse_response;
mod measurement;

use crate::{
    data::{self, SampleRate, Samples, Window},
    icon, log, ui,
};

use iced::{
    alignment::{Horizontal, Vertical},
    keyboard,
    widget::{
        button, center, column, container, horizontal_rule, opaque, pick_list, row, scrollable,
        stack, text, text::Wrapping, Button,
    },
    Alignment, Color, Element, Length, Subscription, Task, Theme,
};
use prism::{axis, line_series, Axis, Chart, Labels};
use raumklang_core::dbfs;

use std::{collections::HashMap, fmt::Display, sync::Arc};

pub struct Main {
    state: State,
    selected: Option<measurement::Selected>,
    smoothing: frequency_response::Smoothing,
    loopback: Option<ui::Loopback>,
    measurements: Vec<ui::Measurement>,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    CollectingMeasuremnts,
    Analysing {
        active_tab: Tab,
        window: Window<Samples>,
        selected_impulse_response: Option<ui::measurement::Id>,
        impulse_responses: HashMap<ui::measurement::Id, ui::impulse_response::State>,
        frequency_responses: HashMap<ui::measurement::Id, frequency_response::Item>,
        charts: Charts,
    },
}

#[derive(Debug, Default)]
struct Charts {
    impulse_response: impulse_response::Chart,
    frequency_response: frequency_response::ChartData,
}

// #[derive(Default)]
// enum Modal {
//     #[default]
//     None,
//     PendingWindow {
//         goto_tab: Tab,
//     },
//     ReplaceLoopback {
//         loopback: data::measurement::State<data::measurement::Loopback>,
//     },
// }

// #[derive(Debug, Clone)]
// pub enum ModalAction {
//     Discard,
//     Apply,
// }

#[derive(Debug, Clone)]
pub enum Message {
    TabSelected(Tab),
    Measurements(measurement::Message),
    ImpulseResponseSelected(ui::measurement::Id),
    ImpulseResponseComputed((ui::measurement::Id, ui::ImpulseResponse)),
    FrequencyResponseToggled(ui::measurement::Id, bool),
    SmoothingChanged(frequency_response::Smoothing),
    FrequencyResponseComputed((ui::measurement::Id, raumklang_core::FrequencyResponse)),
    FrequencyResponseSmoothed((ui::measurement::Id, Box<[f32]>)),
    ImpulseResponseChart(impulse_response::ChartOperation),
    ShiftKeyPressed,
    ShiftKeyReleased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tab {
    Measurements,
    ImpulseResponses,
    FrequencyResponses,
}

impl Main {
    pub fn from_project(project: data::Project) -> (Self, Task<Message>) {
        let load_loopback = project
            .loopback
            .map(|loopback| {
                Task::perform(
                    measurement::load_measurement(loopback.0.path, measurement::Kind::Loopback),
                    measurement::Message::Loaded,
                )
            })
            .unwrap_or(Task::none());

        let load_measurements = project.measurements.into_iter().map(|measurement| {
            Task::perform(
                measurement::load_measurement(measurement.path, measurement::Kind::Normal),
                measurement::Message::Loaded,
            )
        });

        (
            Self::default(),
            Task::batch([load_loopback, Task::batch(load_measurements)]).map(Message::Measurements),
        )
    }

    pub fn update(&mut self, message: Message) -> Task<Message> {
        match message {
            Message::TabSelected(tab) => {
                let State::Analysing {
                    ref mut active_tab,
                    ref impulse_responses,
                    ref mut frequency_responses,
                    ref window,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                *active_tab = tab;

                if let Tab::FrequencyResponses = tab {
                    let impulse_response_tasks = impulse_responses
                        .iter()
                        .filter_map(|(id, state)| {
                            let computed = state.computed()?;
                            Some((id, computed))
                        })
                        .flat_map(|(id, impulse_response)| {
                            if let Some(entry) = frequency_responses.get(&id) {
                                if matches!(
                                    entry.state,
                                    frequency_response::State::ComputingFrequencyResponse
                                        | frequency_response::State::Computed(_)
                                ) {
                                    return None;
                                }
                            };

                            let (frequency_response, computation) =
                                ui::frequency_response::State::new(
                                    *id,
                                    impulse_response.clone(),
                                    window.clone(),
                                );

                            frequency_responses.insert(
                                *id,
                                frequency_response::Item::from_state(frequency_response),
                            );

                            Some(computation)
                        })
                        .map(|computation| {
                            Task::perform(computation.run(), Message::FrequencyResponseComputed)
                        });

                    let missing_impulse_responses_tasks = self
                        .measurements
                        .iter()
                        .filter(|m| m.is_loaded())
                        .filter(|m| impulse_responses.get(&m.id).is_none())
                        .map(|m| Task::done(Message::ImpulseResponseSelected(m.id)));

                    Task::batch([
                        Task::batch(impulse_response_tasks),
                        Task::batch(missing_impulse_responses_tasks),
                    ])
                } else {
                    Task::none()
                }
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
                    measurement::Message::Loaded(Ok(result)) => {
                        match Arc::into_inner(result) {
                            Some(measurement::LoadedKind::Loopback(loopback)) => {
                                self.loopback = Some(ui::Loopback::from_data(loopback))
                            }
                            Some(measurement::LoadedKind::Normal(measurement)) => self
                                .measurements
                                .push(ui::Measurement::from_data(measurement)),
                            None => {}
                        }

                        Task::none()
                    }
                    measurement::Message::Loaded(Err(err)) => {
                        log::error!("{err}");
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
                        window: Window::new(SampleRate::from(
                            self.loopback
                                .as_ref()
                                .and_then(|l| l.inner.loaded())
                                .map_or(44_100, |l| l.as_ref().sample_rate()),
                        ))
                        .into(),
                        selected_impulse_response: None,
                        impulse_responses: HashMap::new(),
                        frequency_responses: HashMap::new(),
                        charts: Charts::default(),
                    },
                    (old_state, true) => old_state,
                    (State::Analysing { .. }, false) => State::CollectingMeasuremnts,
                };

                task.map(Message::Measurements)
            }
            Message::ImpulseResponseSelected(id) => {
                let State::Analysing {
                    selected_impulse_response,
                    impulse_responses,
                    frequency_responses,
                    ..
                } = &mut self.state
                else {
                    return Task::none();
                };

                *selected_impulse_response = Some(id);

                if impulse_responses.contains_key(&id) {
                    Task::none()
                } else {
                    let (impulse_response, computation) = ui::impulse_response::State::new(
                        id,
                        self.loopback
                            .as_ref()
                            .and_then(|l| l.inner.loaded())
                            .unwrap()
                            .clone(),
                        self.measurements
                            .iter()
                            .find(|m| m.id == id)
                            .and_then(|m| m.inner.loaded())
                            .unwrap()
                            .clone(),
                    );

                    impulse_responses.insert(id, impulse_response.clone());
                    frequency_responses.insert(
                        id,
                        frequency_response::Item::from_impulse_response_state(impulse_response),
                    );

                    Task::perform(computation.run(), Message::ImpulseResponseComputed)
                }
            }
            Message::ImpulseResponseComputed((id, impulse_response)) => {
                let State::Analysing {
                    window,
                    active_tab,
                    impulse_responses,
                    frequency_responses,
                    ..
                } = &mut self.state
                else {
                    return Task::none();
                };

                impulse_responses
                    .entry(id)
                    .and_modify(|ir| ir.set_computed(impulse_response.clone()));

                if let Tab::FrequencyResponses = active_tab {
                    let (frequency_response, computation) =
                        ui::frequency_response::State::new(id, impulse_response, window.clone());

                    frequency_responses
                        .insert(id, frequency_response::Item::from_state(frequency_response));

                    Task::perform(computation.run(), Message::FrequencyResponseComputed)
                } else {
                    Task::none()
                }
            }
            Message::ImpulseResponseChart(chart_operation) => {
                let State::Analysing {
                    charts: ref mut chart_data,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                chart_data.impulse_response.update(chart_operation);

                Task::none()
            }
            Message::FrequencyResponseComputed((id, frequency_response)) => {
                let State::Analysing {
                    ref mut frequency_responses,
                    charts: ref mut chart_data,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                frequency_responses.entry(id).and_modify(|entry| {
                    *entry = frequency_response::Item::from_data(frequency_response);

                    chart_data.frequency_response.cache.clear();
                });

                Task::none()
            }
            Message::FrequencyResponseToggled(id, state) => {
                let State::Analysing {
                    ref mut frequency_responses,
                    charts: ref mut chart_data,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                frequency_responses
                    .entry(id)
                    .and_modify(|entry| entry.is_shown = state);

                chart_data.frequency_response.cache.clear();

                Task::none()
            }
            Message::SmoothingChanged(smoothing) => {
                let State::Analysing {
                    ref mut frequency_responses,
                    charts: ref mut chart_data,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                self.smoothing = smoothing;

                if let Some(fraction) = smoothing.fraction() {
                    let tasks = frequency_responses
                        .iter()
                        .flat_map(|(id, fr)| {
                            if let frequency_response::State::Computed(fr) = &fr.state {
                                Some((*id, fr.clone()))
                            } else {
                                None
                            }
                        })
                        .map(|(id, frequency_response)| {
                            Task::perform(
                                frequency_response::smooth_frequency_response(
                                    id,
                                    frequency_response,
                                    fraction,
                                ),
                                Message::FrequencyResponseSmoothed,
                            )
                        });

                    Task::batch(tasks)
                } else {
                    frequency_responses
                        .values_mut()
                        .for_each(|entry| entry.smoothed = None);

                    chart_data.frequency_response.cache.clear();

                    Task::none()
                }
            }
            Message::FrequencyResponseSmoothed((id, smoothed_data)) => {
                let State::Analysing {
                    ref mut frequency_responses,
                    charts: ref mut chart_data,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                frequency_responses
                    .entry(id)
                    .and_modify(|entry| entry.smoothed = Some(smoothed_data));

                chart_data.frequency_response.cache.clear();

                Task::none()
            }
            Message::ShiftKeyPressed => {
                let State::Analysing {
                    charts: ref mut chart_data,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                chart_data.impulse_response.shift_key_pressed();

                Task::none()
            }
            Message::ShiftKeyReleased => {
                let State::Analysing {
                    charts: ref mut chart_data,
                    ..
                } = self.state
                else {
                    return Task::none();
                };

                chart_data.impulse_response.shift_key_released();

                Task::none()
            }
        }
    }

    pub fn view(&self) -> Element<Message> {
        let header = container(match &self.state {
            State::CollectingMeasuremnts => Tab::Measurements.view(false),
            State::Analysing { active_tab, .. } => active_tab.view(true),
        })
        .width(Length::Fill)
        .style(container::dark);

        let content = match &self.state {
            State::CollectingMeasuremnts => self.measurements_tab().map(Message::Measurements),
            State::Analysing {
                active_tab,
                impulse_responses,
                selected_impulse_response,
                frequency_responses,
                charts: chart_data,
                ..
            } => match active_tab {
                Tab::Measurements => self.measurements_tab().map(Message::Measurements),
                Tab::ImpulseResponses => self.impulse_responses_tab(
                    *selected_impulse_response,
                    &chart_data.impulse_response,
                    impulse_responses,
                ),
                Tab::FrequencyResponses => self
                    .frequency_responses_tab(frequency_responses, &chart_data.frequency_response),
            },
        };

        container(column![header, container(content).padding(5)].spacing(10)).into()
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

        let content: Element<_> = {
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
    pub fn impulse_responses_tab<'a>(
        &'a self,
        selected: Option<ui::measurement::Id>,
        chart: &'a impulse_response::Chart,
        impulse_responses: &'a HashMap<ui::measurement::Id, ui::impulse_response::State>,
    ) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                column!(text("For Measurements"), horizontal_rule(1))
                    .width(Length::Fill)
                    .spacing(5)
            };

            let entries = self
                .measurements
                .iter()
                .filter(|m| m.is_loaded())
                .map(|measurement| (measurement, impulse_responses.get(&measurement.id)))
                .map(|(measurement, ir)| {
                    let id = measurement.id;

                    let entry = {
                        let content = column![text(&measurement.name).size(16),]
                            .spacing(5)
                            .clip(true)
                            .spacing(3);

                        button(content)
                            .on_press_with(move || Message::ImpulseResponseSelected(id))
                            .width(Length::Fill)
                            .style(move |theme, status| {
                                let status = match selected {
                                    Some(selected) if selected == id => button::Status::Hovered,
                                    _ => status,
                                };
                                button::secondary(theme, status)
                            })
                            .into()
                    };

                    if let Some(ir) = ir {
                        match &ir {
                            ui::impulse_response::State::Computing => {
                                impulse_response::processing_overlay("Impulse Response", entry)
                            }
                            ui::impulse_response::State::Computed(_) => entry,
                        }
                    } else {
                        entry
                    }
                });

            container(scrollable(
                column![header, column(entries).spacing(3)]
                    .spacing(10)
                    .padding(10),
            ))
            .style(container::rounded_box)
        };

        let content = {
            if let Some(impulse_response) = selected
                .and_then(|id| impulse_responses.get(&id))
                .and_then(|state| state.computed())
            {
                chart
                    .view(impulse_response)
                    .map(Message::ImpulseResponseChart)
            } else {
                container(text("Impulse response not computed, yet.")).into()
            }
        };

        row![
            container(sidebar).width(Length::FillPortion(1)),
            container(content).center(Length::FillPortion(4))
        ]
        .spacing(10)
        .into()
    }

    fn frequency_responses_tab<'a>(
        &'a self,
        frequency_responses: &'a HashMap<ui::measurement::Id, frequency_response::Item>,
        chart_settings: &'a frequency_response::ChartData,
    ) -> Element<'a, Message> {
        let sidebar = {
            let entries =
                self.measurements
                    .iter()
                    .filter(|m| m.is_loaded())
                    .flat_map(|measurement| {
                        let name = &measurement.name;
                        frequency_responses.get(&measurement.id).map(|item| {
                            item.view(name, |state| {
                                Message::FrequencyResponseToggled(measurement.id, state)
                            })
                        })
                    });

            container(column(entries).spacing(10).padding(8)).style(container::rounded_box)
        };

        let header = {
            row![pick_list(
                frequency_response::Smoothing::ALL,
                Some(&self.smoothing),
                Message::SmoothingChanged,
            )]
        };

        let content = if frequency_responses.values().any(|item| item.is_shown) {
            let series_list = frequency_responses
                .values()
                .filter(|item| item.is_shown)
                .filter(|item| matches!(item.state, frequency_response::State::Computed(_)))
                .flat_map(|item| {
                    let frequency_response::State::Computed(frequency_response) = &item.state
                    else {
                        return [None, None];
                    };
                    let sample_rate = frequency_response.sample_rate;
                    let len = frequency_response.data.len() * 2 + 1;
                    let resolution = sample_rate as f32 / len as f32;

                    let closure = move |(i, s)| (i as f32 * resolution, dbfs(s));

                    [
                        Some(
                            line_series(
                                frequency_response
                                    .data
                                    .iter()
                                    .copied()
                                    .enumerate()
                                    .map(closure),
                            )
                            .color(item.color.scale_alpha(0.1)),
                        ),
                        item.smoothed.as_ref().map(|smoothed| {
                            line_series(smoothed.iter().copied().enumerate().map(closure))
                                .color(item.color)
                        }),
                    ]
                })
                .flatten();

            let chart: Chart<Message, ()> = Chart::new()
                .x_axis(
                    Axis::new(axis::Alignment::Horizontal)
                        .scale(axis::Scale::Log)
                        .x_tick_marks(
                            [0, 20, 50, 100, 1000, 10_000, 20_000]
                                .into_iter()
                                .map(|v| v as f32)
                                .collect(),
                        ),
                )
                .x_range(chart_settings.x_range.clone().unwrap_or(20.0..=22_500.0))
                .y_labels(Labels::default().format(&|v| format!("{v:.0}")))
                .extend_series(series_list)
                .cache(&chart_settings.cache);
            // .on_scroll(|state| {
            //     let pos = state.get_coords();
            //     let delta = state.scroll_delta();
            //     let x_range = state.x_range();
            //     Message::Chart(ChartOperation::Scroll(pos, delta, x_range))
            // });

            container(chart)
        } else {
            container(text("Please select a frequency respone."))
        };

        row![
            container(sidebar)
                .width(Length::FillPortion(3))
                .style(container::bordered_box),
            column![header, container(content).width(Length::FillPortion(10))].spacing(12)
        ]
        .spacing(10)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let hotkeys_pressed = keyboard::on_key_press(|key, _modifiers| {
            use keyboard::key::{Key, Named};

            Some(match key.as_ref() {
                Key::Named(Named::Shift) => Message::ShiftKeyPressed,
                _ => None?,
            })
        });

        let hotkeys_released = keyboard::on_key_release(|key, _modifiers| {
            use keyboard::key::{Key, Named};

            Some(match key.as_ref() {
                Key::Named(Named::Shift) => Message::ShiftKeyReleased,
                _ => None?,
            })
        });

        Subscription::batch([hotkeys_pressed, hotkeys_released])
    }

    fn analysing_possible(&self) -> bool {
        self.loopback.as_ref().is_some_and(ui::Loopback::is_loaded)
            && self.measurements.iter().any(ui::Measurement::is_loaded)
    }
}

impl Default for Main {
    fn default() -> Self {
        Self {
            state: State::CollectingMeasuremnts,
            selected: None,
            smoothing: frequency_response::Smoothing::default(),
            loopback: None,
            measurements: vec![],
        }
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
        let mut row = row![];

        for tab in Tab::iter() {
            let is_active = self == tab;

            let is_enabled = match tab {
                Tab::Measurements => true,
                Tab::ImpulseResponses | Tab::FrequencyResponses => is_analysing,
            };

            let button = button(text(tab.to_string()).size(16))
                .padding(10)
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

        row.spacing(5).align_y(Alignment::Center).into()
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
