use crate::{
    data,
    widgets::{charts::frequency_response, colored_circle},
};

use iced::{
    widget::{column, container, horizontal_space, row, stack, text, toggler},
    Alignment, Color, Element,
    Length::{self, FillPortion},
    Task,
};
use plotters::style::{Color as _, Palette, Palette99, RGBAColor};
use rand::Rng;

use std::{collections::HashMap, iter};

use super::compute_impulse_response;

#[derive(Debug, Clone)]
pub enum Message {
    ListEntry(data::MeasurementId, ListEntryMessage),
    Chart(frequency_response::Message),
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
    chart: Option<frequency_response::FrequencyResponseChart>,
}

enum EntryState {
    Loading {
        name: String,
        show_in_graph: bool,
        color: RGBAColor,
    },
    Loaded {
        name: String,
        show_in_graph: bool,
        color: RGBAColor,
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

        let responses = entries
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
            .flat_map(|(id, color)| frequency_responses.get(&id).map(|fr| (fr.clone(), color)))
            .map(|(fr, color)| frequency_response::FrequencyResponseData::new(fr, color));

        let chart = frequency_response::FrequencyResponseChart::from_iter(responses);

        (Self { entries, chart }, Task::batch(tasks))
    }

    pub fn view(&self) -> Element<'_, Message> {
        let entries = self
            .entries
            .iter()
            .map(|(i, e)| e.view().map(move |msg| Message::ListEntry(*i, msg)));

        let list = container(column(entries).spacing(10).padding(8).width(FillPortion(1)))
            .style(container::rounded_box);

        let content = if let Some(chart) = &self.chart {
            container(chart.view().map(Message::Chart))
        } else {
            container(text("Please select a frequency respone.")).center(Length::FillPortion(4))
        }
        .padding(8)
        .width(Length::FillPortion(4));

        row![list, content].into()
    }

    pub fn update(
        &mut self,
        message: Message,
        frequency_responses: &HashMap<data::MeasurementId, raumklang_core::FrequencyResponse>,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::ListEntry(id, message) => {
                let Some(entry) = self.entries.get_mut(&id) else {
                    return (Task::none(), None);
                };

                entry.update(message);

                if let Some(chart) = &mut self.chart {
                    let responses = self
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
                        .flat_map(|(id, color)| {
                            frequency_responses.get(&id).map(|fr| (fr.clone(), color))
                        })
                        .map(|(fr, color)| {
                            frequency_response::FrequencyResponseData::new(fr, color)
                        });

                    chart.update_data(responses);
                }

                (Task::none(), None)
            }
            Message::Chart(message) => {
                let Some(chart) = &mut self.chart else {
                    return (Task::none(), None);
                };

                chart.update(message);
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
                let Some(entry) = self.entries.get_mut(&id) else {
                    return (Task::none(), None);
                };

                let cur_color = match entry {
                    EntryState::Loading {
                        name,
                        show_in_graph,
                        color,
                    } => {
                        let color = *color;

                        *entry = EntryState::Loaded {
                            name: name.clone(),
                            show_in_graph: *show_in_graph,
                            color,
                            frequency_response_id: id,
                        };

                        color
                    }
                    EntryState::Loaded { color, .. } => *color,
                };

                let responses = self
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
                    .flat_map(|(id, color)| {
                        frequency_responses.get(&id).map(|fr| (fr.clone(), color))
                    })
                    .chain(iter::once((fr.clone(), cur_color)))
                    .map(|(fr, color)| frequency_response::FrequencyResponseData::new(fr, color));

                if let Some(chart) = &mut self.chart {
                    chart.update_data(responses);
                } else {
                    self.chart = frequency_response::FrequencyResponseChart::from_iter(responses);
                }

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
                let color = Color::from_rgba8(color.0, color.1, color.2, color.3 as f32);
                let content = column![
                    text(name),
                    row![
                        toggler(*show_in_graph)
                            //.on_toggle(ListEntryMessage::ShowInGraphToggled)
                            .width(Length::Shrink),
                        horizontal_space(),
                        colored_circle(10.0, color),
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
                let color = Color::from_rgba8(color.0, color.1, color.2, color.3 as f32);
                let content = column![
                    text(name),
                    row![
                        toggler(*show_in_graph)
                            .on_toggle(ListEntryMessage::ShowInGraphToggled)
                            .width(Length::Shrink),
                        horizontal_space(),
                        colored_circle(10.0, color),
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

fn random_color() -> RGBAColor {
    let max = Palette99::COLORS.len();
    let index = rand::thread_rng().gen_range(0..max);
    Palette99::pick(index).to_rgba()
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
