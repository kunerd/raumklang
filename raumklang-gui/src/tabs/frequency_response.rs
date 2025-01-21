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

use std::{collections::HashMap, sync::Arc};

#[derive(Debug, Clone)]
pub enum Message {
    ListEntry(usize, ListEntryMessage),
    Chart(chart::FrequencyResponseChartMessage),
    FrequencyResponseComputed(Arc<raumklang_core::FrequencyResponse>),
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

        for measurement in measurements {
            let id = 0;
            let frequency_response_id = if let Some(_fr) = frequency_responses.get(&id) {
                Some(id)
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
                    tasks.push(
                        Task::future(compute_impulse_response(id, loopback, measurement)).then(
                            move |(id, ir)| {
                                let window = window.clone();
                                Task::perform(
                                    compute_frequency_response(id, ir, window),
                                    Message::FrequencyResponseComputed,
                                )
                            },
                        ),
                    );
                };

                None
            };

            entries.push(ListEntry {
                name: measurement.name.clone(),
                show_in_graph: true,
                frequency_response_id,
            });
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

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ListEntry(id, message) => {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.update(message)
                }
            }
            Message::Chart(message) => {
                let Some(chart) = &mut self.chart else {
                    return;
                };

                chart.update(message);
            }
            Message::FrequencyResponseComputed(fr) => {
                self.chart = Some(FrequencyResponseChart::new(fr));
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
    (
        id,
        raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement).unwrap(),
    )
}

async fn compute_frequency_response(
    id: usize,
    impulse_response: raumklang_core::ImpulseResponse,
    window: Vec<f32>,
) -> Arc<raumklang_core::FrequencyResponse> {
    Arc::new(raumklang_core::FrequencyResponse::new(
        impulse_response,
        &window,
    ))
}
