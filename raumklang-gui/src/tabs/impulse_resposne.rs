use std::sync::Arc;

use iced::{
    widget::{button, column, container, row, text},
    Element, Length, Task,
};
use iced_aw::TabLabel;

use crate::{widgets::chart::{self, TimeSeriesUnit, TimeseriesChart}, Signal, Signals};

use super::Tab;

#[derive(Default)]
pub struct ImpulseResponse {
    chart: Option<TimeseriesChart>,
}

#[derive(Debug, Clone)]
pub enum Message {
    MeasurementSignalSelected,
    ImpulseResponseComputed(Arc<raumklang_core::ImpulseResponse>),
    TimeSeriesChart(chart::Message),
}

impl ImpulseResponse {
    pub fn update(&mut self, msg: Message, signals: &Signals) -> Task<Message> {
        match msg {
            Message::MeasurementSignalSelected => {
                if let (Some(loopback), Some(response)) =
                    (&signals.loopback, &signals.measurements.first())
                {
                    Task::perform(
                        compute_impulse_response(loopback.data.clone(), response.data.clone()),
                        Message::ImpulseResponseComputed,
                    )
                } else {
                    Task::none()
                }
            }
            Message::ImpulseResponseComputed(impulse_response) => {
                let data = impulse_response.impulse_response.iter().map(|c| c.norm()).collect();
                let signal = Signal::new("Impulse response".to_string(), 44100, data);

                self.chart = Some(TimeseriesChart::new(
                    signal,
                    TimeSeriesUnit::Time,
                ));

                Task::none()
            }
            Message::TimeSeriesChart(_) => {
                Task::none()
            },
        }
    }
}

async fn compute_impulse_response(
    loopback: Vec<f32>,
    response: Vec<f32>,
) -> Arc<raumklang_core::ImpulseResponse> {
    Arc::new(raumklang_core::ImpulseResponse::from_signals(loopback, response).unwrap())
}

impl Tab for ImpulseResponse {
    type Message = Message;

    fn title(&self) -> String {
        "Impulse Response".to_string()
    }

    fn label(&self) -> iced_aw::TabLabel {
        TabLabel::Text(self.title())
    }

    fn content<'a>(&'a self, signals: &'a crate::Signals) -> iced::Element<'a, Self::Message> {
        let side_menu: Element<'_, Message> = {
            let loopback_entry = {
                let header = text("Loopback");
                let entry: Element<_> = if let Some(signal) = &signals.loopback {
                    signal_list_entry(signal)
                } else {
                    text("Please load a loopback signal, first!".to_string()).into()
                };

                column!(header, entry).width(Length::Fill).spacing(5)
            };

            let measurement_entry = {
                let header = text("Measurements");
                let entry: Element<'_, Message> =
                    if let Some(signal) = &signals.measurements.first() {
                        button(signal_list_entry(signal))
                            .on_press(Message::MeasurementSignalSelected)
                            .style(button::secondary)
                            .into()
                    } else {
                        text("Please load a measurent signal, first!".to_string()).into()
                    };

                column!(header, entry).width(Length::Fill).spacing(5)
            };

            container(column!(loopback_entry, measurement_entry).spacing(10))
                .padding(5)
                .width(Length::FillPortion(1))
                .into()
        };

        let content = {
            if let Some(chart) = &self.chart {
                container(chart.view().map(Message::TimeSeriesChart))
                    .width(Length::FillPortion(5))
            } else {
                container(text("Not implemented.".to_string()))
            }
        };

        row!(side_menu, content).into()
    }
}

// FIXME duplicated code
fn signal_list_entry(signal: &Signal) -> Element<'_, Message> {
    let samples = signal.data.len();
    let sample_rate = signal.sample_rate as f32;
    column!(
        text(&signal.name),
        text(format!("Samples: {}", samples)),
        text(format!("Duration: {} s", samples as f32 / sample_rate)),
    )
    .padding(2)
    .into()
}
