use crate::{
    data,
    widgets::chart::{self, FrequencyResponseChart},
};

use iced::{
    widget::{column, container, row, text, toggler},
    Element,
    Length::{self, FillPortion},
    Task,
};

use std::collections::HashMap;

#[derive(Debug, Clone)]
pub enum Message {
    ListEntry(usize, ListEntryMessage),
    Chart(chart::FrequencyResponseChartMessage),
    FrequencyResponseComputed((usize, raumklang_core::FrequencyResponse)),
    ImpulseResponseComputed((usize, raumklang_core::ImpulseResponse)),
}

pub enum Event {
    ImpulseResponseComputed(usize, raumklang_core::ImpulseResponse),
    FrequencyResponseComputed(usize, raumklang_core::FrequencyResponse),
}

#[derive(Debug, Clone)]
pub enum ListEntryMessage {
    ShowInGraphToggled(bool),
}

pub struct FrequencyResponse {
    entries: Vec<ListEntry>,
    chart: Option<FrequencyResponseChart>,
}

#[derive(Default)]
struct ListEntry {
    name: String,
    show_in_graph: bool,
    frequency_response_id: Option<usize>,
}

impl FrequencyResponse {
    pub fn new<'a>(
        loopback: &'a data::Loopback,
        measurements: impl Iterator<Item = &'a data::Measurement>,
        impulse_responses: &'a HashMap<usize, raumklang_core::ImpulseResponse>,
        frequency_responses: &'a HashMap<usize, raumklang_core::FrequencyResponse>,
    ) -> (Self, Task<Message>) {
        let (_, size_hint) = measurements.size_hint();
        let mut entries = Vec::with_capacity(size_hint.unwrap_or(10));
        let mut tasks = vec![];

        for (id, measurement) in measurements.enumerate() {
            if let Some(_fr) = frequency_responses.get(&id) {
                entries.push(ListEntry {
                    name: measurement.name.clone(),
                    show_in_graph: false,
                    frequency_response_id: Some(id),
                });
            } else {
                let window = raumklang_core::WindowBuilder::default().build();

                let loopback = loopback.0.data.clone();
                let measurement = measurement.data.clone();

                if let Some(ir) = impulse_responses.get(&id) {
                    tasks.push(Task::perform(
                        compute_frequency_response(id, ir.clone(), window),
                        Message::FrequencyResponseComputed,
                    ));
                } else {
                    tasks.push(Task::perform(
                        compute_impulse_response(id, loopback, measurement),
                        Message::ImpulseResponseComputed,
                    ));
                };
            };
        }

        (
            Self {
                entries,
                chart: None,
            },
            Task::batch(tasks),
        )
    }

    pub fn view(&self) -> Element<'_, Message> {
        let entries = self
            .entries
            .iter()
            .enumerate()
            .map(|(i, e)| e.view().map(move |msg| Message::ListEntry(i, msg)));

        let list = container(column(entries).spacing(5).padding(8).width(FillPortion(1)))
            .style(container::rounded_box);

        let content = if let Some(chart) = &self.chart {
            container(chart.view().map(Message::Chart))
        } else {
            container(text("Please select a frequency respone.")).center(Length::FillPortion(4))
        }
        .padding(10)
        .width(Length::FillPortion(4));

        row![list, content].padding(10).into()
    }

    pub fn update(
        &mut self,
        message: Message,
        frequency_responses: &HashMap<usize, raumklang_core::FrequencyResponse>,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::ListEntry(id, message) => {
                let Some(entry) = self.entries.get_mut(id) else {
                    return (Task::none(), None);
                };

                if let ListEntryMessage::ShowInGraphToggled(true) = &message {
                    self.chart = frequency_responses
                        .get(&entry.frequency_response_id.unwrap())
                        .cloned()
                        .map(FrequencyResponseChart::new);
                }

                entry.update(message);

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
                self.entries.push(ListEntry {
                    name: "Fixme".to_string(),
                    show_in_graph: false,
                    frequency_response_id: Some(id),
                });

                self.chart = Some(FrequencyResponseChart::new(fr.clone()));
                (Task::none(), Some(Event::FrequencyResponseComputed(id, fr)))
            }
        }
    }
}

impl ListEntry {
    fn view(&self) -> Element<'_, ListEntryMessage> {
        let content = column![
            text(&self.name),
            toggler(self.show_in_graph).on_toggle(ListEntryMessage::ShowInGraphToggled)
        ];
        container(content).style(container::rounded_box).into()
    }

    fn update(&mut self, message: ListEntryMessage) {
        match message {
            ListEntryMessage::ShowInGraphToggled(state) => self.show_in_graph = state,
        }
    }
}

async fn compute_impulse_response(
    id: usize,
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
) -> (usize, raumklang_core::ImpulseResponse) {
    let impulse_response = tokio::task::spawn_blocking(move || {
        raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement).unwrap()
    })
    .await
    .unwrap();

    (id, impulse_response)
}

async fn compute_frequency_response(
    id: usize,
    impulse_response: raumklang_core::ImpulseResponse,
    window: Vec<f32>,
) -> (usize, raumklang_core::FrequencyResponse) {
    let frequency_response = tokio::task::spawn_blocking(move || {
        raumklang_core::FrequencyResponse::new(impulse_response, &window)
    })
    .await
    .unwrap();

    (id, frequency_response)
}
