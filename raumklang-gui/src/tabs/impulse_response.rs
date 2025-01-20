use std::collections::HashMap;

use iced::{
    widget::{button, column, container, horizontal_rule, horizontal_space, row, scrollable, text},
    Alignment, Element,
    Length::{self, FillPortion},
    Task,
};

use crate::{
    data,
    widgets::chart::{self, ImpulseResponseChart, TimeSeriesUnit},
    OfflineMeasurement,
};

#[derive(Debug, Clone)]
pub enum Message {
    MeasurementSelected(usize),
    ImpulseResponseComputed((usize, raumklang_core::ImpulseResponse)),
    Chart(chart::Message),
    //    ImpulseResponseComputed((Arc<raumklang_core::ImpulseResponse>, u32)),
    //    TimeSeriesChart(chart::Message),
    //    TabSelected(TabId),
    //    WindowSettings(window_settings::Message),
    //    FrequencyResponseChart(FrequencyResponseChartMessage),
    //    FrequencyResponseComputed(Arc<FrequencyResponse>),
}

pub enum Event {
    ImpulseResponseComputed(usize, raumklang_core::ImpulseResponse),
}

#[derive(Default)]
pub struct ImpulseResponseTab {
    selected: Option<usize>,
    chart: Option<ImpulseResponseChart>,
    //    active_tab: TabId,
    //    loopback_signal: Option<Measurement>,
    //    measurement_signal: Option<Measurement>,
    //    window_settings: Option<WindowSettings>,
    //    impulse_response: Option<raumklang_core::ImpulseResponse>,
    //    frequency_response: Option<Arc<FrequencyResponse>>,
    //    frequency_response_chart: Option<FrequencyResponseChart>,
}

impl ImpulseResponseTab {
    pub fn update(
        &mut self,
        message: Message,
        loopback: &data::Loopback,
        measurements: &data::Store<data::Measurement, OfflineMeasurement>,
        impulse_response: &HashMap<usize, raumklang_core::ImpulseResponse>,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::MeasurementSelected(id) => {
                self.selected = Some(id);
                if let Some(ir) = impulse_response.get(&id) {
                    self.update_chart(ir);
                    (Task::none(), None)
                } else {
                    let measurement = measurements.get_loaded(&id);
                    if let Some(measurement) = measurement {
                        (
                            Task::perform(
                                compute_impulse_response(
                                    id,
                                    loopback.0.data.clone(),
                                    measurement.data.clone(),
                                ),
                                Message::ImpulseResponseComputed,
                            ),
                            None,
                        )
                    } else {
                        (Task::none(), None)
                    }
                }
            }
            Message::ImpulseResponseComputed((id, ir)) => {
                self.update_chart(&ir);
                (Task::none(), Some(Event::ImpulseResponseComputed(id, ir)))
            }
            Message::Chart(message) => {
                let Some(chart) = &mut self.chart else {
                    return (Task::none(), None);
                };

                chart.update_msg(message);
                (Task::none(), None)
            }
        }
    }

    pub fn view<'a>(&'a self, measurements: &[&'a data::Measurement]) -> Element<'a, Message> {
        let list = {
            let entries: Vec<Element<_>> = measurements
                .iter()
                .enumerate()
                .map(|(i, m)| {
                    let style = if self.selected == Some(i) {
                        button::primary
                    } else {
                        button::secondary
                    };

                    button(m.name.as_str())
                        .on_press(Message::MeasurementSelected(i))
                        .style(style)
                        .width(Length::Fill)
                        .into()
                })
                .collect();

            let content = scrollable(column(entries).spacing(5)).into();
            container(list_category("Measurements", content))
                .style(container::rounded_box)
                .height(Length::Fill)
                .padding(8)
        };

        let content = if let Some(chart) = &self.chart {
            container(chart.view().map(Message::Chart))
        } else {
            container(text(
                "Please select a measurement to compute the corresponding impulse response.",
            ))
            .center(Length::Fill)
        };

        row![
            list.width(Length::FillPortion(1)),
            content.width(FillPortion(4))
        ]
        .spacing(10)
        .into()
    }

    fn update_chart(&mut self, ir: &raumklang_core::ImpulseResponse) {
        self.chart = Some(ImpulseResponseChart::new(ir.clone(), TimeSeriesUnit::Time));
    }
}

fn list_category<'a>(name: &'a str, content: Element<'a, Message>) -> Element<'a, Message> {
    let header = row!(text(name), horizontal_space()).align_y(Alignment::Center);

    column!(header, horizontal_rule(1), content)
        .width(Length::Fill)
        .spacing(5)
        .into()
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

//
//#[derive(Debug, Clone, PartialEq, Eq, Default)]
//pub enum TabId {
//    #[default]
//    ImpulseResponse,
//    FrequencyResponse,
//}
//
//impl ImpulseResponseTab {
//    pub fn update(&mut self, msg: Message) -> Task<Message> {
//        match msg {
//            Message::ImpulseResponseComputed((impulse_response, sample_rate)) => {
//                let data: Vec<_> = impulse_response
//                    .impulse_response
//                    .iter()
//                    .map(|s| s.re().abs())
//                    .collect();
//
//                let signal =
//                    Measurement::new("Impulse response".to_string(), sample_rate, data.clone());
//                self.impulse_response_chart =
//                    Some(ImpulseResponseChart::new(signal, TimeSeriesUnit::Time));
//                self.impulse_response = Some(Arc::into_inner(impulse_response).unwrap());
//
//                self.recompute_window();
//                self.recompute_frequency_response()
//            }
//            Message::TimeSeriesChart(msg) => {
//                if let Some(chart) = &mut self.impulse_response_chart {
//                    chart.update_msg(msg);
//                }
//
//                Task::none()
//            }
//            Message::TabSelected(id) => {
//                self.active_tab = id;
//
//                self.recompute_frequency_response()
//            }
//            Message::FrequencyResponseComputed(fr) => {
//                //let data = fr.data.iter().map(|s| dbfs(s.norm())).collect();
//                // FIXME stupid use of Arc
//                let fr = Arc::into_inner(fr).unwrap();
//                self.frequency_response = Some(Arc::new(fr.clone()));
//                self.frequency_response_chart = Some(FrequencyResponseChart::new(fr));
//                Task::none()
//            }
//            Message::FrequencyResponseChart(msg) => {
//                if let Some(chart) = self.frequency_response_chart.as_mut() {
//                    chart.update(msg);
//                }
//
//                Task::none()
//            }
//            Message::WindowSettings(msg) => {
//                let Some(settings) = &mut self.window_settings else {
//                    return Task::none();
//                };
//
//                settings.update(msg);
//
//                self.recompute_window();
//
//                Task::none()
//            }
//        }
//    }
//
//    pub fn view(&self) -> Element<'_, Message> {
//        let content = {
//            let impulse_response = {
//                if let Some(chart) = &self.impulse_response_chart {
//                    let col = column![];
//
//                    let col = col
//                        .push_maybe(
//                            self.window_settings
//                                .as_ref()
//                                .map(|s| s.view().map(Message::WindowSettings)),
//                        )
//                        .push(chart.view().map(Message::TimeSeriesChart));
//
//                    container(col).width(Length::FillPortion(5))
//                } else {
//                    container(text("No measurement selected."))
//                }
//            };
//
//            let frequency_response: Element<'_, Message> =
//                if let Some(chart) = &self.frequency_response_chart {
//                    chart.view().map(Message::FrequencyResponseChart)
//                } else {
//                    text("You need to select a measurement and setup a window, first.").into()
//                };
//
//            Tabs::new(Message::TabSelected)
//                .push(
//                    TabId::ImpulseResponse,
//                    TabLabel::Text("Impulse Response".to_string()),
//                    impulse_response,
//                )
//                .push(
//                    TabId::FrequencyResponse,
//                    TabLabel::Text("Frequency Response".to_string()),
//                    frequency_response,
//                )
//                .set_active_tab(&self.active_tab)
//                .tab_bar_position(iced_aw::TabBarPosition::Top)
//        };
//
//        content.into()
//    }
//
//    pub fn loopback_signal_changed(&mut self, signal: Measurement) -> Task<Message> {
//        let max_width = signal.data.len();
//        self.loopback_signal = Some(signal);
//        self.window_settings = Some(WindowSettings::new(WindowBuilder::default(), max_width));
//        self.compute_impulse_response()
//    }
//
//    pub fn set_selected_measurement(&mut self, signal: Measurement) -> Task<Message> {
//        self.measurement_signal = Some(signal);
//        self.compute_impulse_response()
//    }
//
//    fn compute_impulse_response(&self) -> Task<Message> {
//        if let (Some(loopback), Some(response)) = (
//            self.loopback_signal.clone(),
//            self.measurement_signal.clone(),
//        ) {
//            Task::perform(
//                compute_impulse_response(loopback, response),
//                Message::ImpulseResponseComputed,
//            )
//        } else {
//            Task::none()
//        }
//    }
//
//    fn recompute_window(&mut self) {
//        let window = self
//            .window_settings
//            .as_ref()
//            .map_or(vec![], |s| s.window_builder.build());
//
//        if let Some(chart) = &mut self.impulse_response_chart {
//            chart.update_window(window);
//        }
//    }
//
//    fn recompute_frequency_response(&self) -> Task<Message> {
//        let Some(loopback) = &self.loopback_signal else {
//            return Task::none();
//        };
//
//        let Some(settings) = &self.window_settings else {
//            return Task::none();
//        };
//
//        let (TabId::FrequencyResponse, Some(ir)) = (&self.active_tab, &self.impulse_response)
//        else {
//            return Task::none();
//        };
//        let ir = ir.clone();
//        Task::perform(
//            compute_frequency_response(ir, loopback.sample_rate, settings.window_builder.build()),
//            Message::FrequencyResponseComputed,
//        )
//    }
//}
//
////impl Tab for ImpulseResponseTab {
////    type Message = Message;
////
////    fn title(&self) -> String {
////        "Impulse Response".to_string()
////    }
////
////    fn label(&self) -> iced_aw::TabLabel {
////        TabLabel::Text(self.title())
////    }
////
////    fn content(&self) -> iced::Element<'_, Self::Message> {
////        let content = {
////            let impulse_response = {
////                if let Some(chart) = &self.impulse_response_chart {
////                    let col = column![];
////
////                    let col = col
////                        .push_maybe(
////                            self.window_settings
////                                .as_ref()
////                                .map(|s| s.view().map(Message::WindowSettings)),
////                        )
////                        .push(chart.view().map(Message::TimeSeriesChart));
////
////                    container(col).width(Length::FillPortion(5))
////                } else {
////                    container(text("No measurement selected."))
////                }
////            };
////
////            let frequency_response: Element<'_, Message> =
////                if let Some(chart) = &self.frequency_response_chart {
////                    chart.view().map(Message::FrequencyResponseChart)
////                } else {
////                    text("You need to select a measurement and setup a window, first.").into()
////                };
////
////            Tabs::new(Message::TabSelected)
////                .push(
////                    TabId::ImpulseResponse,
////                    TabLabel::Text("Impulse Response".to_string()),
////                    impulse_response,
////                )
////                .push(
////                    TabId::FrequencyResponse,
////                    TabLabel::Text("Frequency Response".to_string()),
////                    frequency_response,
////                )
////                .set_active_tab(&self.active_tab)
////                .tab_bar_position(iced_aw::TabBarPosition::Top)
////        };
////
////        content.into()
////    }
////}
//
//async fn compute_frequency_response(
//    impulse_response: raumklang_core::ImpulseResponse,
//    sample_rate: u32,
//    window: Vec<f32>,
//) -> Arc<FrequencyResponse> {
//    Arc::new(FrequencyResponse::new(
//        impulse_response,
//        sample_rate,
//        &window,
//    ))
//}
//
//async fn compute_impulse_response(
//    loopback: Measurement,
//    response: Measurement,
//) -> (Arc<raumklang_core::ImpulseResponse>, u32) {
//    (
//        Arc::new(
//            raumklang_core::ImpulseResponse::from_signals(loopback.data, response.data).unwrap(),
//        ),
//        loopback.sample_rate,
//    )
//}
//
//async fn windowed_median(data: &mut [f32]) -> f32 {
//    const WINDOW_SIZE: usize = 512;
//
//    let mut mean_of_median = 0f32;
//    let window_count = data.len() / WINDOW_SIZE;
//
//    for window_num in 0..window_count {
//        let start = window_num * WINDOW_SIZE;
//        let end = start + WINDOW_SIZE;
//
//        let window = &mut data[start..end];
//        window.sort_by(|a, b| a.partial_cmp(b).unwrap());
//
//        mean_of_median += window[256];
//    }
//
//    mean_of_median / window_count as f32
//}
//
//async fn estimate_noise_floor_border(noise_floor: f32, data: &[f32]) -> usize {
//    const WINDOW_SIZE: usize = 1024 * 2;
//
//    let window_count = data.len() / WINDOW_SIZE;
//    let nf_border = 0;
//
//    for window_num in 0..window_count {
//        let start = window_num * WINDOW_SIZE;
//        let end = start + WINDOW_SIZE;
//
//        let window = &data[start..end];
//
//        let mean = window.iter().fold(0f32, |acc, e| acc + e) / WINDOW_SIZE as f32;
//        if mean < noise_floor {
//            return end;
//        }
//    }
//
//    nf_border
//}
