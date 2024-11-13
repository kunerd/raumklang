use std::sync::Arc;

use iced::{
    futures::FutureExt,
    widget::{container, text},
    Element, Length, Task,
};
use iced_aw::{TabLabel, Tabs};
use raumklang_core::dbfs;
use rustfft::{num_complex::Complex32, FftPlanner};

use crate::{
    widgets::chart::{self, FrequencyResponseChart, TimeSeriesUnit, TimeseriesChart},
    window::{Window, WindowBuilder},
    Signal, SignalState, Signals,
};

use super::Tab;

#[derive(Debug, Clone)]
pub enum Message {
    MeasurementSignalSelected(usize),
    ImpulseResponseComputed(Arc<raumklang_core::ImpulseResponse>),
    TimeSeriesChart(chart::Message),
    TabSelected(TabId),
    FrequencyResponseComputed(Arc<FrequencyResponse>),
    FrequencyResponseChart(()),
}

#[derive(Default)]
pub struct ImpulseResponse {
    active_tab: TabId,
    chart: Option<TimeseriesChart>,
    impulse_response: Option<raumklang_core::ImpulseResponse>,
    frequency_response: Option<Arc<FrequencyResponse>>,
    frequency_response_chart: Option<FrequencyResponseChart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TabId {
    #[default]
    ImpulseResponse,
    FrequencyResponse,
}

#[derive(Debug)]
pub struct FrequencyResponse {
    data: Vec<Complex32>,
}

impl FrequencyResponse {
    pub fn new(impulse_response: raumklang_core::ImpulseResponse, window: &[f32]) -> Self {
        let mut windowed_impulse_response: Vec<_> = impulse_response
            .impulse_response
            .iter()
            .take(window.len())
            .enumerate()
            .map(|(i, s)| s * window[i])
            .collect();

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(windowed_impulse_response.len());

        fft.process(&mut windowed_impulse_response);

        let data_len = windowed_impulse_response.len() / 2;
        let data = windowed_impulse_response
            .into_iter()
            .take(data_len)
            .collect();

        Self {
            data,
        }
    }
}

impl ImpulseResponse {
    pub fn update(&mut self, msg: Message, signals: &Signals) -> Task<Message> {
        match msg {
            Message::MeasurementSignalSelected(id) => {
                if let (Some(SignalState::Loaded(loopback)), Some(SignalState::Loaded(response))) =
                    (&signals.loopback, &signals.measurements.get(id))
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
                let data: Vec<_> = impulse_response
                    .impulse_response
                    .iter()
                    .map(|c| dbfs(c.norm()))
                    .collect();

                let signal = Signal::new("Impulse response".to_string(), 44100, data.clone());
                self.chart = Some(TimeseriesChart::new(signal, TimeSeriesUnit::Time));
                self.impulse_response = Some(Arc::into_inner(impulse_response).unwrap());

                Task::perform(
                    async move {
                        let mut data = data.clone();
                        let nf = windowed_median(&mut data).await;
                        let nfb = estimate_noise_floor_border(nf, &data).await;
                        (nf, nfb)
                    }
                    .map(chart::Message::NoiseFloorUpdated),
                    Message::TimeSeriesChart,
                )
            }
            Message::TimeSeriesChart(msg) => {
                if let Some(chart) = &mut self.chart {
                    chart.update_msg(msg);
                }

                Task::none()
            }
            Message::TabSelected(id) => {
                self.active_tab = id.clone();

                if let (TabId::FrequencyResponse, Some(ir)) = (id, &self.impulse_response) {
                    let ir = ir.clone();
                    Task::perform(
                        compute_frequency_response(ir),
                        Message::FrequencyResponseComputed,
                    )
                } else {
                    Task::none()
                }
            }
            Message::FrequencyResponseComputed(fr) => {
                self.frequency_response = Some(fr.clone());
                let data = fr.data.iter().map(|s| dbfs(s.norm())).collect();
                self.frequency_response_chart = Some(FrequencyResponseChart::new(data));
                Task::none()
            }
            Message::FrequencyResponseChart(_) => todo!(),
        }
    }
}

impl Tab for ImpulseResponse {
    type Message = Message;

    fn title(&self) -> String {
        "Impulse Response".to_string()
    }

    fn label(&self) -> iced_aw::TabLabel {
        TabLabel::Text(self.title())
    }

    fn content(&self) -> iced::Element<'_, Self::Message> {
        let content = {
            let impulse_response = {
                if let Some(chart) = &self.chart {
                    container(chart.view().map(Message::TimeSeriesChart))
                        .width(Length::FillPortion(5))
                } else {
                    container(text("Not implemented.".to_string()))
                }
            };

            let frequency_response: Element<'_, Message> =
                if let Some(chart) = &self.frequency_response_chart {
                    chart.view().map(Message::FrequencyResponseChart)
                } else {
                    text("Not computed, yet!").into()
                };

            Tabs::new(Message::TabSelected)
                .push(
                    TabId::ImpulseResponse,
                    TabLabel::Text("Impulse Response".to_string()),
                    impulse_response,
                )
                .push(
                    TabId::FrequencyResponse,
                    TabLabel::Text("Frequency Response".to_string()),
                    frequency_response,
                )
                .set_active_tab(&self.active_tab)
                .tab_bar_position(iced_aw::TabBarPosition::Top)
        };

        content.into()
    }
}

async fn compute_frequency_response(
    impulse_response: raumklang_core::ImpulseResponse,
) -> Arc<FrequencyResponse> {
    let window = WindowBuilder::new(Window::Tukey(0.25), Window::Tukey(0.25), 4000).build();
    Arc::new(FrequencyResponse::new(impulse_response, &window))
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
