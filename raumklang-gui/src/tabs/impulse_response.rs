use std::sync::Arc;

use iced::{
    futures::FutureExt,
    widget::{column, container, pick_list, row, text, text_input},
    Element, Length, Task,
};
use iced_aw::{number_input, TabLabel, Tabs};
use rustfft::{
    num_complex::{Complex32, ComplexFloat},
    FftPlanner,
};

use crate::{
    widgets::chart::{
        self, FrequencyResponseChart, FrequencyResponseChartMessage, ImpulseResponseChart,
        TimeSeriesUnit,
    },
    window::{Window, WindowBuilder},
    Measurement,
};

use super::Tab;

#[derive(Debug, Clone)]
pub enum Message {
    ImpulseResponseComputed((Arc<raumklang_core::ImpulseResponse>, u32)),
    TimeSeriesChart(chart::Message),
    TabSelected(TabId),
    LeftWindowChanged(Window),
    LeftWindowWidthChanged(usize),
    RightWindowChanged(Window),
    RightWindowWidthChanged(usize),
    WindowWidthChanged(usize),
    FrequencyResponseChart(FrequencyResponseChartMessage),
    FrequencyResponseComputed(Arc<FrequencyResponse>),
}

#[derive(Default)]
pub struct ImpulseResponseTab {
    active_tab: TabId,
    loopback_signal: Option<Measurement>,
    measurement_signal: Option<Measurement>,
    window_builder: WindowBuilder,
    impulse_response: Option<raumklang_core::ImpulseResponse>,
    impulse_response_chart: Option<ImpulseResponseChart>,
    frequency_response: Option<Arc<FrequencyResponse>>,
    frequency_response_chart: Option<FrequencyResponseChart>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum TabId {
    #[default]
    ImpulseResponse,
    FrequencyResponse,
}

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub sample_rate: u32,
    pub data: Vec<Complex32>,
}

impl FrequencyResponse {
    pub fn new(
        impulse_response: raumklang_core::ImpulseResponse,
        sample_rate: u32,
        window: &[f32],
    ) -> Self {
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

        let data_len = windowed_impulse_response.len() / 2 - 1;
        let data = windowed_impulse_response
            .into_iter()
            .take(data_len)
            .collect();

        // FIXME fix constant sample rate
        Self { sample_rate, data }
    }
}

impl ImpulseResponseTab {
    pub fn update(&mut self, msg: Message) -> Task<Message> {
        match msg {
            Message::ImpulseResponseComputed((impulse_response, sample_rate)) => {
                let data: Vec<_> = impulse_response
                    .impulse_response
                    .iter()
                    .map(|s| s.re().abs())
                    .collect();

                let signal =
                    Measurement::new("Impulse response".to_string(), sample_rate, data.clone());
                self.impulse_response_chart =
                    Some(ImpulseResponseChart::new(signal, TimeSeriesUnit::Time));
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
                if let Some(chart) = &mut self.impulse_response_chart {
                    chart.update_msg(msg);
                }

                Task::none()
            }
            Message::TabSelected(id) => {
                let Some(loopback) = &self.loopback_signal else {
                    return Task::none();
                };

                let (TabId::FrequencyResponse, Some(ir)) = (&id, &self.impulse_response) else {
                    return Task::none();
                };
                self.active_tab = id;
                let ir = ir.clone();
                Task::perform(
                    compute_frequency_response(
                        ir,
                        loopback.sample_rate,
                        self.window_builder.build(),
                    ),
                    Message::FrequencyResponseComputed,
                )
            }
            Message::FrequencyResponseComputed(fr) => {
                //let data = fr.data.iter().map(|s| dbfs(s.norm())).collect();
                // FIXME stupid use of Arc
                let fr = Arc::into_inner(fr).unwrap();
                self.frequency_response = Some(Arc::new(fr.clone()));
                self.frequency_response_chart = Some(FrequencyResponseChart::new(fr));
                Task::none()
            }
            Message::FrequencyResponseChart(msg) => {
                if let Some(chart) = self.frequency_response_chart.as_mut() {
                    chart.update(msg);
                }

                Task::none()
            }
            Message::LeftWindowChanged(selected) => {
                self.window_builder.set_left_side(selected);
                self.recompute_window();
                Task::none()
            }
            Message::RightWindowChanged(selected) => {
                self.window_builder.set_right_side(selected);
                self.recompute_window();
                Task::none()
            }
            Message::LeftWindowWidthChanged(width) => {
                self.window_builder.set_left_side_width(width);
                self.recompute_window();
                Task::none()
            }
            Message::RightWindowWidthChanged(width) => {
                self.window_builder.set_right_side_width(width);
                self.recompute_window();
                Task::none()
            }
            Message::WindowWidthChanged(width) => {
                self.window_builder.set_width(width);
                self.recompute_window();
                Task::none()
            }
        }
    }

    pub fn loopback_signal_changed(&mut self, signal: Measurement) -> Task<Message> {
        self.loopback_signal = Some(signal);
        self.compute_impulse_response()
    }

    pub fn set_selected_measurement(&mut self, signal: Measurement) -> Task<Message> {
        self.measurement_signal = Some(signal);
        self.compute_impulse_response()
    }

    fn compute_impulse_response(&self) -> Task<Message> {
        if let (Some(loopback), Some(response)) = (
            self.loopback_signal.clone(),
            self.measurement_signal.clone(),
        ) {
            Task::perform(
                compute_impulse_response(loopback, response),
                Message::ImpulseResponseComputed,
            )
        } else {
            Task::none()
        }
    }

    fn recompute_window(&mut self) {
        let window = self.window_builder.build();

        if let Some(chart) = &mut self.impulse_response_chart {
            chart.update_window(window);
        }
    }
}

const ALL_WINDOW_TYPE: [Window; 2] = [Window::Hann, Window::Tukey(0.25)];

impl Tab for ImpulseResponseTab {
    type Message = Message;

    fn title(&self) -> String {
        "Impulse Response".to_string()
    }

    fn label(&self) -> iced_aw::TabLabel {
        TabLabel::Text(self.title())
    }

    fn content(&self) -> iced::Element<'_, Self::Message> {
        let max_window_width = self
            .impulse_response
            .as_ref()
            .map_or_else(|| 44_100, |ir| ir.impulse_response.len());

        let content = {
            let impulse_response = {
                if let Some(chart) = &self.impulse_response_chart {
                    let window_settings = {
                        let left_window_settings =
                            maybe_window_settings_view(&self.window_builder.left_side());
                        let right_window_settings =
                            maybe_window_settings_view(&self.window_builder.right_side());

                        column![
                            row![
                                text("Left hand window:"),
                                pick_list(
                                    ALL_WINDOW_TYPE,
                                    Some(self.window_builder.left_side()),
                                    Message::LeftWindowChanged
                                ),
                                text("width:"),
                                number_input(
                                    self.window_builder.left_side_width(),
                                    0..self.window_builder.max_left_side_width(),
                                    Message::LeftWindowWidthChanged
                                )
                            ]
                            .push_maybe(left_window_settings),
                            row![
                                text("Right hand window:"),
                                pick_list(
                                    ALL_WINDOW_TYPE,
                                    Some(self.window_builder.right_side()),
                                    Message::RightWindowChanged,
                                ),
                                text("width:"),
                                number_input(
                                    self.window_builder.right_side_width(),
                                    0..self.window_builder.max_right_side_width(),
                                    Message::RightWindowWidthChanged
                                )
                            ]
                            .push_maybe(right_window_settings),
                            row![
                                text("Window width"),
                                number_input(
                                    self.window_builder.width(),
                                    0..max_window_width,
                                    Message::WindowWidthChanged
                                )
                            ]
                        ]
                    };

                    let col = column!(window_settings, chart.view().map(Message::TimeSeriesChart));
                    container(col).width(Length::FillPortion(5))
                } else {
                    container(text("No measurement selected."))
                }
            };

            let frequency_response: Element<'_, Message> =
                if let Some(chart) = &self.frequency_response_chart {
                    chart.view().map(Message::FrequencyResponseChart)
                } else {
                    text("You need to select a measurement and setup a window, first.").into()
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

fn maybe_window_settings_view(window: &Window) -> Option<Element<'static, Message>> {
    match window {
        Window::Hann => None,
        Window::Tukey(alpha) => Some(text_input("alpha", format!("{alpha}").as_str())),
    }
    .map(|v| v.into())
}

async fn compute_frequency_response(
    impulse_response: raumklang_core::ImpulseResponse,
    sample_rate: u32,
    window: Vec<f32>,
) -> Arc<FrequencyResponse> {
    Arc::new(FrequencyResponse::new(
        impulse_response,
        sample_rate,
        &window,
    ))
}

async fn compute_impulse_response(
    loopback: Measurement,
    response: Measurement,
) -> (Arc<raumklang_core::ImpulseResponse>, u32) {
    (
        Arc::new(
            raumklang_core::ImpulseResponse::from_signals(loopback.data, response.data).unwrap(),
        ),
        loopback.sample_rate,
    )
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
