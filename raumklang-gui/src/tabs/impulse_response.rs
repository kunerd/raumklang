use std::collections::HashMap;

use iced::{
    widget::{
        button, checkbox, column, container, horizontal_rule, horizontal_space, row, scrollable,
        text,
    },
    Alignment, Element,
    Length::{self, FillPortion},
    Task,
};
use raumklang_core::WindowBuilder;

use crate::{
    components::window_settings::{self, WindowSettings},
    data,
    widgets::charts::{impulse_response, TimeSeriesUnit},
    OfflineMeasurement,
};

use super::compute_impulse_response;

#[derive(Debug, Clone)]
pub enum Message {
    MeasurementSelected(data::MeasurementId),
    ImpulseResponseComputed((data::MeasurementId, raumklang_core::ImpulseResponse)),
    Chart(impulse_response::Message),
    ShowWindowToggled(bool),
    WindowSettings(window_settings::Message),
}

pub enum Event {
    ImpulseResponseComputed(data::MeasurementId, raumklang_core::ImpulseResponse),
}

#[derive(Default)]
pub struct ImpulseResponseTab {
    chart: Option<impulse_response::ImpulseResponseChart>,
    selected: Option<data::MeasurementId>,
    show_window: bool,
    window_settings: Option<WindowSettings>,
}

impl ImpulseResponseTab {
    pub fn update(
        &mut self,
        message: Message,
        loopback: &data::Loopback,
        measurements: &data::Store<data::Measurement, OfflineMeasurement>,
        impulse_response: &HashMap<data::MeasurementId, raumklang_core::ImpulseResponse>,
    ) -> (Task<Message>, Option<Event>) {
        match message {
            Message::MeasurementSelected(id) => {
                self.selected = Some(id);

                if let Some(ir) = impulse_response.get(&id) {
                    self.update_chart(ir);
                    self.window_settings =
                        Some(WindowSettings::new(WindowBuilder::default(), ir.data.len()));

                    (Task::none(), None)
                } else {
                    let measurement = measurements.get_loaded_by_id(&id);
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
            Message::ShowWindowToggled(state) => {
                self.show_window = state;

                if let (Some(chart), Some(window_settings)) =
                    (&mut self.chart, &mut self.window_settings)
                {
                    let maybe_window = if self.show_window {
                        Some(window_settings.window_builder.build())
                    } else {
                        None
                    };
                    chart.set_window(maybe_window);
                }

                (Task::none(), None)
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
            Message::WindowSettings(msg) => {
                let Some(window_settings) = &mut self.window_settings else {
                    return (Task::none(), None);
                };

                window_settings.update(msg);

                if let Some(chart) = &mut self.chart {
                    chart.set_window(Some(window_settings.window_builder.build()));
                }

                (Task::none(), None)
            }
        }
    }

    pub fn view<'a>(
        &'a self,
        measurements: impl Iterator<Item = (&'a data::MeasurementId, &'a data::Measurement)>,
    ) -> Element<'a, Message> {
        let list = {
            let entries: Vec<Element<_>> = measurements
                .map(|(i, m)| {
                    let style = if self.selected == Some(*i) {
                        button::primary
                    } else {
                        button::secondary
                    };

                    button(m.name.as_str())
                        .on_press(Message::MeasurementSelected(*i))
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
            container(
                column![
                    checkbox("Show Window", self.show_window).on_toggle(Message::ShowWindowToggled),
                    chart.view().map(Message::Chart),
                ]
                .push_maybe(
                    self.window_settings
                        .as_ref()
                        .map(|s| s.view().map(Message::WindowSettings)),
                ),
            )
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
        self.chart = Some(impulse_response::ImpulseResponseChart::new(
            ir.clone(),
            TimeSeriesUnit::Time,
        ));
    }
}

fn list_category<'a>(name: &'a str, content: Element<'a, Message>) -> Element<'a, Message> {
    let header = row!(text(name), horizontal_space()).align_y(Alignment::Center);

    column!(header, horizontal_rule(1), content)
        .width(Length::Fill)
        .spacing(5)
        .into()
}

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
