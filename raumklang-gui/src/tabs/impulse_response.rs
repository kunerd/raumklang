use std::collections::HashMap;

use iced::{
    advanced::graphics::geometry,
    widget::{
        button, canvas, checkbox, column, container, horizontal_rule, horizontal_space, pick_list,
        row, scrollable, text,
    },
    Alignment, Element,
    Length::{self, FillPortion},
    Task,
};
use pliced::widget::line_series;
use plotters_iced::Renderer;
use raumklang_core::WindowBuilder;

use crate::{
    components::window_settings::{self, WindowSettings},
    data,
    widgets::charts::{
        impulse_response::{self, ImpulseResponseChart},
        AmplitudeUnit, TimeSeriesUnit,
    },
    OfflineMeasurement,
};

use super::compute_impulse_response;

#[derive(Debug, Clone)]
pub enum Message {
    MeasurementSelected(data::MeasurementId),
    ImpulseResponseComputed((data::MeasurementId, raumklang_core::ImpulseResponse)),
    ShowWindowToggled(bool),
    WindowSettings(window_settings::Message),
    Chart(Operation),
    RawChart(impulse_response::Message),
}

pub enum Event {
    ImpulseResponseComputed(data::MeasurementId, raumklang_core::ImpulseResponse),
}

#[derive(Default)]
pub struct ImpulseResponseTab {
    selected: Option<data::MeasurementId>,
    show_window: bool,
    window_settings: Option<WindowSettings>,
    chart_data: ChartData,
}

#[derive(Default)]
pub struct ChartData {
    amplitude_unit: AmplitudeUnit,
    time_unit: TimeSeriesUnit,
    cache: canvas::Cache,
}

impl ChartData {
    fn apply(&mut self, operation: Operation) {
        match operation {
            Operation::TimeUnitChanged(time_unit) => self.time_unit = time_unit,
            Operation::AmplitudeUnitChanged(amplitude_unit) => self.amplitude_unit = amplitude_unit,
        }

        self.cache.clear();
    }
}

#[derive(Debug, Clone)]
pub enum Operation {
    TimeUnitChanged(TimeSeriesUnit),
    AmplitudeUnitChanged(AmplitudeUnit),
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

                (Task::none(), None)
            }
            Message::ImpulseResponseComputed((id, ir)) => {
                (Task::none(), Some(Event::ImpulseResponseComputed(id, ir)))
            }
            Message::Chart(operation) => {
                self.chart_data.apply(operation);

                (Task::none(), None)
            }
            Message::WindowSettings(msg) => {
                let Some(window_settings) = &mut self.window_settings else {
                    return (Task::none(), None);
                };

                window_settings.update(msg);

                (Task::none(), None)
            }
            Message::RawChart(_message) => (Task::none(), None),
        }
    }

    pub fn view<'a>(
        &'a self,
        measurements: impl Iterator<Item = (&'a data::MeasurementId, &'a data::Measurement)>,
        impulse_responses: &'a HashMap<data::MeasurementId, raumklang_core::ImpulseResponse>,
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

        let content = if let Some(impulse_response) = self
            .selected
            .as_ref()
            .and_then(|id| impulse_responses.get(id))
        {
            let chart_menu = row![
                text("Amplitude unit:"),
                pick_list(
                    &AmplitudeUnit::ALL[..],
                    Some(&self.chart_data.amplitude_unit),
                    |unit| Message::Chart(Operation::AmplitudeUnitChanged(unit))
                ),
                text("Time unit:"),
                pick_list(
                    &TimeSeriesUnit::ALL[..],
                    Some(&self.chart_data.time_unit),
                    |unit| { Message::Chart(Operation::TimeUnitChanged(unit)) }
                ),
                checkbox("Show Window", self.show_window).on_toggle(Message::ShowWindowToggled),
            ]
            .align_y(Alignment::Center)
            .spacing(10);

            let chart = pliced::widget::Chart::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .push_series(
                    line_series(
                        impulse_response
                            .data
                            .iter()
                            .enumerate()
                            .map(|(i, s)| (i as f32, s.re.abs())),
                    )
                    .color(iced::Color::from_rgb8(2, 125, 66).into()),
                );

            container(
                column![chart_menu, chart].push_maybe(
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
