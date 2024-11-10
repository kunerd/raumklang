use std::sync::Arc;

use iced::{
    futures::FutureExt, widget::{button, column, container, row, text}, Element, Length, Task
};
use iced_aw::TabLabel;
use raumklang_core::dbfs;

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
                let data: Vec<_> = impulse_response.impulse_response.iter().map(|c| dbfs(c.norm())).collect();
                let signal = Signal::new("Impulse response".to_string(), 44100, data.clone());


                self.chart = Some(TimeseriesChart::new(
                    signal,
                    TimeSeriesUnit::Time,
                ));

                Task::perform(async move {
                    let mut data = data.clone();
                    let nf = windowed_median(&mut data).await;
                    let nfb = estimate_noise_floor_border(nf, &data).await;
                    println!("Noise floor border: {nfb}");
                    nf
                }.map(chart::Message::NoiseFloorUpdated), Message::TimeSeriesChart)
            }
            Message::TimeSeriesChart(msg) => {
                if let Some(chart) = &mut self.chart {
                    chart.update_msg(msg);
                }
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

async fn windowed_median(data: &mut [f32]) -> f32 {
    const WINDOW_SIZE: usize = 512;

    let mut mean_of_median = 0f32;
    let window_count = data.len() / WINDOW_SIZE;

    for window_num in 0..window_count {
        let start = window_num * WINDOW_SIZE;
        let end = start + WINDOW_SIZE;
        
        let window = &mut data[start..end];
        window.sort_by(|a, b| a.partial_cmp(b).unwrap());

        mean_of_median += window[256];
    }

    mean_of_median / window_count as f32
}

async fn estimate_noise_floor_border(noise_floor: f32, data: &[f32]) -> usize {
    const WINDOW_SIZE: usize = 1024 * 2;

    let window_count = data.len() / WINDOW_SIZE;
    let nf_border = 0;

    for window_num in 0..window_count {
        let start = window_num * WINDOW_SIZE;
        let end = start + WINDOW_SIZE;
        
        let window = &data[start..end];

        let mean = window.iter().fold(0f32, |acc, e| acc + e) / WINDOW_SIZE as f32;
        if mean < noise_floor {
            return end;
        }
    }

    nf_border
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
