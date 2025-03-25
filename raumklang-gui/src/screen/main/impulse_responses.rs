use iced::{
    widget::{
        button, column, container, horizontal_rule, horizontal_space, pick_list, row, scrollable,
        text,
    },
    Alignment, Element, Length,
};
use pliced::chart::{line_series, Chart, Labels};

use crate::data::{self, chart, impulse_response};

pub struct ImpulseReponses {
    selected: Option<usize>,
    chart_data: ChartData,
}

#[derive(Default)]
pub struct ChartData {
    amplitude_unit: chart::AmplitudeUnit,
    time_unit: chart::TimeSeriesUnit,
}

#[derive(Debug, Clone)]
pub enum Message {
    Select(usize),
    Chart(ChartOperation),
}

#[derive(Debug, Clone)]
pub enum ChartOperation {
    TimeUnitChanged(chart::TimeSeriesUnit),
    AmplitudeUnitChanged(chart::AmplitudeUnit),
}

pub enum Action {
    ComputeImpulseResponse(usize),
    None,
}

impl ImpulseReponses {
    pub fn new() -> Self {
        Self {
            selected: None,
            chart_data: ChartData::default(),
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Select(id) => {
                self.selected = Some(id);

                Action::ComputeImpulseResponse(id)
            }
            Message::Chart(operation) => {
                self.chart_data.apply(operation);

                Action::None
            }
        }
    }

    pub fn view<'a>(&'a self, measurements: &'a [data::Measurement]) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                column!(text("For Measurements"), horizontal_rule(1))
                    .width(Length::Fill)
                    .spacing(5)
            };

            let measurements = measurements.iter().enumerate().map(|(id, entry)| {
                let content = column![text(&entry.name).size(16),]
                    .spacing(5)
                    .clip(true)
                    .spacing(3);

                let style = match self.selected.as_ref() {
                    Some(selected) if *selected == id => button::primary,
                    _ => button::secondary,
                };

                button(content)
                    .on_press_with(move || Message::Select(id))
                    .width(Length::Fill)
                    .style(style)
                    .into()
            });

            container(scrollable(
                column![header, column(measurements).spacing(3)]
                    .spacing(10)
                    .padding(10),
            ))
            .style(container::rounded_box)
        }
        .width(Length::FillPortion(1));

        let content: Element<_> = {
            if let Some(id) = self.selected {
                let state = measurements
                    .get(id)
                    .map(|m| &m.state)
                    .and_then(|s| match s {
                        data::measurement::MeasurementState::Loaded {
                            impulse_response: impulse_response::State::Computed(impulse_response),
                            ..
                        } => Some(impulse_response),
                        _ => None,
                    });

                match state {
                    Some(impulse_response) => {
                        let header = row![pick_list(
                            &chart::AmplitudeUnit::ALL[..],
                            Some(&self.chart_data.amplitude_unit),
                            |unit| Message::Chart(ChartOperation::AmplitudeUnitChanged(unit))
                        ),]
                        .align_y(Alignment::Center)
                        .spacing(10);

                        let x_scale_fn = match self.chart_data.time_unit {
                            chart::TimeSeriesUnit::Samples => sample_scale,
                            chart::TimeSeriesUnit::Time => time_scale,
                        };

                        let y_scale_fn: fn(f32, f32) -> f32 = match self.chart_data.amplitude_unit {
                            chart::AmplitudeUnit::PercentFullScale => percent_full_scale,
                            chart::AmplitudeUnit::DezibelFullScale => db_full_scale,
                        };

                        let sample_rate = impulse_response.sample_rate as f32;

                        let chart: Chart<_, (), _> = Chart::new()
                            .width(Length::Fill)
                            .height(Length::Fill)
                            .x_range(
                                x_scale_fn(-44_10.0, sample_rate)
                                    ..=x_scale_fn(44_100.0, sample_rate),
                            )
                            .y_labels(Labels::default().format(&|v| format!("{v:.2}")))
                            .push_series(
                                line_series(impulse_response.data.iter().enumerate().map(
                                    move |(i, s)| {
                                        (
                                            x_scale_fn(i as f32, sample_rate),
                                            y_scale_fn(*s, impulse_response.max),
                                        )
                                    },
                                ))
                                .color(iced::Color::from_rgb8(2, 125, 66)),
                            );

                        let footer = {
                            row![
                                horizontal_space(),
                                pick_list(
                                    &chart::TimeSeriesUnit::ALL[..],
                                    Some(&self.chart_data.time_unit),
                                    |unit| {
                                        Message::Chart(ChartOperation::TimeUnitChanged(unit))
                                    }
                                ),
                            ]
                            .align_y(Alignment::Center)
                        };

                        container(column![header, chart, footer]).into()
                    }
                    // TODO: add spinner
                    None => text("Impulse response not computed, yet.").into(),
                }
            } else {
                text("Please select an entry to view its data.").into()
            }
        };

        row![
            container(sidebar).width(Length::FillPortion(1)),
            container(content).center(Length::FillPortion(4))
        ]
        .spacing(10)
        .into()
    }
}

impl ChartData {
    fn apply(&mut self, operation: ChartOperation) {
        match operation {
            ChartOperation::TimeUnitChanged(time_unit) => self.time_unit = time_unit,
            ChartOperation::AmplitudeUnitChanged(amplitude_unit) => {
                self.amplitude_unit = amplitude_unit
            }
        }
    }
}

fn percent_full_scale(s: f32, max: f32) -> f32 {
    s / max * 100f32
}

fn db_full_scale(s: f32, max: f32) -> f32 {
    let y = 20f32 * f32::log10(s.abs() / max);
    y.clamp(-100.0, max)
}

fn sample_scale(index: f32, _sample_rate: f32) -> f32 {
    index
}

fn time_scale(index: f32, sample_rate: f32) -> f32 {
    index / sample_rate * 1000.0
}
