use super::compute_impulse_response;
use crate::{data, widgets::colored_circle};

use pliced::chart::{line_series, Chart};
use raumklang_core::dbfs;

use iced::{
    widget::{column, container, horizontal_space, row, stack, text, toggler},
    Alignment, Color, Element,
    Length::{self, FillPortion},
    Task,
};
use rand::Rng;

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Message {
    ListEntry(data::MeasurementId, ListEntryMessage),
    FrequencyResponseComputed((data::MeasurementId, raumklang_core::FrequencyResponse)),
    ImpulseResponseComputed((data::MeasurementId, raumklang_core::ImpulseResponse)),
}

pub enum Event {
    ImpulseResponseComputed(data::MeasurementId, raumklang_core::ImpulseResponse),
    FrequencyResponseComputed(data::MeasurementId, raumklang_core::FrequencyResponse),
}

#[derive(Debug, Clone)]
pub enum ListEntryMessage {
    ShowInGraphToggled(bool),
}

pub struct FrequencyResponse {
    entries: HashMap<data::MeasurementId, EntryState>,
}

enum EntryState {
    Loading {
        name: String,
        show_in_graph: bool,
        color: iced::Color,
    },
    Loaded {
        name: String,
        show_in_graph: bool,
        color: iced::Color,
        frequency_response_id: data::MeasurementId,
    },
}

impl FrequencyResponse {
    pub fn new<'a>(
        loopback: &'a data::Loopback,
        measurements: impl Iterator<Item = (&'a data::MeasurementId, &'a data::Measurement)>,
        impulse_responses: &'a HashMap<data::MeasurementId, raumklang_core::ImpulseResponse>,
        frequency_responses: &'a HashMap<data::MeasurementId, raumklang_core::FrequencyResponse>,
    ) -> (Self, Task<Message>) {
        let (_, size_hint) = measurements.size_hint();

        let mut entries = HashMap::with_capacity(size_hint.unwrap_or(10));
        let mut tasks = vec![];

        for (id, measurement) in measurements {
            if let Some(_fr) = frequency_responses.get(id) {
                entries.insert(
                    *id,
                    EntryState::Loaded {
                        name: measurement.name.clone(),
                        show_in_graph: true,
                        color: random_color(),
                        frequency_response_id: *id,
                    },
                );
            } else {
                let window = raumklang_core::WindowBuilder::default().build();

                entries.insert(
                    *id,
                    EntryState::Loading {
                        name: measurement.name.clone(),
                        show_in_graph: true,
                        color: random_color(),
                    },
                );

                let loopback = loopback.0.data.clone();
                let measurement = measurement.data.clone();
                if let Some(ir) = impulse_responses.get(id) {
                    tasks.push(Task::perform(
                        compute_frequency_response(*id, ir.clone(), window),
                        Message::FrequencyResponseComputed,
                    ));
                } else {
                    tasks.push(Task::perform(
                        compute_impulse_response(*id, loopback, measurement),
                        Message::ImpulseResponseComputed,
                    ));
                };
            };
        }

        (Self { entries }, Task::batch(tasks))
    }

    pub fn view<'a>(
        &'a self,
        frequency_responses: &'a HashMap<data::MeasurementId, raumklang_core::FrequencyResponse>,
    ) -> Element<'a, Message> {
        let entries = self
            .entries
            .iter()
            .map(|(i, e)| e.view().map(move |msg| Message::ListEntry(*i, msg)));

        let list = container(column(entries).spacing(10).padding(8).width(FillPortion(1)))
            .style(container::rounded_box);

        let content = if self.entries.values().any(|e| {
            matches!(
                e,
                EntryState::Loaded {
                    show_in_graph: true,
                    ..
                }
            )
        }) {
            let series_list = self
                .entries
                .values()
                .filter_map(|e| match *e {
                    EntryState::Loaded {
                        show_in_graph: true,
                        frequency_response_id,
                        color,
                        ..
                    } => Some((frequency_response_id, color)),
                    _ => None,
                })
                .flat_map(|(id, color)| frequency_responses.get(&id).map(|fr| (fr, color)))
                .map(|(fr, color)| {
                    line_series(
                        fr.data
                            .iter()
                            .enumerate()
                            .map(|(i, s)| (i as f32, dbfs(s.re.abs()))),
                    )
                    .color(color)
                });

            let chart: Chart<_, (), _> = Chart::new()
                .x_range(0.0..=1000.0)
                .extend_series(series_list);

            container(chart)
        } else {
            container(text("Please select a frequency respone.")).center(Length::FillPortion(4))
        }
        .padding(8)
        .width(Length::FillPortion(4));

        row![list, content].into()
    }

    pub fn update(&mut self, message: Message) -> (Task<Message>, Option<Event>) {
        match message {
            Message::ListEntry(id, message) => {
                let Some(entry) = self.entries.get_mut(&id) else {
                    return (Task::none(), None);
                };

                entry.update(message);

                (Task::none(), None)
            }
            Message::ImpulseResponseComputed((id, ir)) => {
                let window = raumklang_core::WindowBuilder::default().build();

                let task = Task::perform(
                    compute_frequency_response(id, ir.clone(), window),
                    Message::FrequencyResponseComputed,
                );

                (task, Some(Event::ImpulseResponseComputed(id, ir)))
            }
            Message::FrequencyResponseComputed((id, fr)) => {
                let Some(EntryState::Loading {
                    name,
                    show_in_graph,
                    color,
                }) = self.entries.remove(&id)
                else {
                    return (Task::none(), None);
                };

                self.entries.insert(
                    id,
                    EntryState::Loaded {
                        name,
                        show_in_graph,
                        color,
                        frequency_response_id: id,
                    },
                );

                (Task::none(), Some(Event::FrequencyResponseComputed(id, fr)))
            }
        }
    }
}

impl EntryState {
    fn view(&self) -> Element<'_, ListEntryMessage> {
        match self {
            EntryState::Loading {
                name,
                show_in_graph,
                color,
            } => {
                let content = column![
                    text(name),
                    row![
                        toggler(*show_in_graph)
                            //.on_toggle(ListEntryMessage::ShowInGraphToggled)
                            .width(Length::Shrink),
                        horizontal_space(),
                        colored_circle(10.0, *color),
                    ]
                    .align_y(Alignment::Center)
                ]
                .spacing(5)
                .padding(5);

                stack([
                    container(content).style(container::bordered_box).into(),
                    container(text("Computing..."))
                        .center(Length::Fill)
                        .style(|theme| container::Style {
                            border: container::rounded_box(theme).border,
                            background: Some(iced::Background::Color(Color::from_rgba(
                                0.0, 0.0, 0.0, 0.8,
                            ))),
                            ..Default::default()
                        })
                        .into(),
                ])
                .into()
            }
            EntryState::Loaded {
                name,
                show_in_graph,
                color,
                frequency_response_id: _,
            } => {
                let content = column![
                    text(name),
                    row![
                        toggler(*show_in_graph)
                            .on_toggle(ListEntryMessage::ShowInGraphToggled)
                            .width(Length::Shrink),
                        horizontal_space(),
                        colored_circle(10.0, *color),
                    ]
                    .align_y(Alignment::Center)
                ]
                .spacing(5)
                .padding(5);

                container(content).style(container::rounded_box).into()
            }
        }
    }
    fn update(&mut self, message: ListEntryMessage) {
        match self {
            EntryState::Loading { show_in_graph, .. }
            | EntryState::Loaded { show_in_graph, .. } => match message {
                ListEntryMessage::ShowInGraphToggled(state) => *show_in_graph = state,
            },
        }
    }
}

fn random_color() -> iced::Color {
    const MAX_COLOR_VALUE: u8 = 255;

    // TODO: replace with color palette
    let red = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let green = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let blue = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);

    iced::Color::from_rgb8(red, green, blue)
}

async fn compute_frequency_response(
    id: data::MeasurementId,
    impulse_response: raumklang_core::ImpulseResponse,
    window: Vec<f32>,
) -> (data::MeasurementId, raumklang_core::FrequencyResponse) {
    let frequency_response = tokio::task::spawn_blocking(move || {
        raumklang_core::FrequencyResponse::new(impulse_response, &window)
    })
    .await
    .unwrap();

    (id, frequency_response)
}
