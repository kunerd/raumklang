use std::ops::RangeInclusive;

use iced::{
    keyboard,
    mouse::ScrollDelta,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, pick_list, row, scrollable,
        text,
    },
    Alignment, Element, Length, Point, Subscription,
};
use pliced::chart::{line_series, Chart, Labels};

use crate::data::{self, chart, impulse_response};

pub struct ImpulseReponses {
    selected: Option<usize>,
    chart_data: ChartData,
}

#[derive(Default)]
pub struct ChartData {
    x_max: Option<f32>,
    x_range: Option<RangeInclusive<f32>>,
    shift_key_pressed: bool,
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
    Scroll(
        Option<Point>,
        Option<ScrollDelta>,
        Option<RangeInclusive<f32>>,
    ),
    ShiftKeyPressed,
    ShiftKeyReleased,
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
                        data::measurement::State::Loaded {
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

                        let chart: Chart<_, (), _> = {
                            let x_scale_fn = match self.chart_data.time_unit {
                                chart::TimeSeriesUnit::Samples => sample_scale,
                                chart::TimeSeriesUnit::Time => time_scale,
                            };

                            let y_scale_fn: fn(f32, f32) -> f32 =
                                match self.chart_data.amplitude_unit {
                                    chart::AmplitudeUnit::PercentFullScale => percent_full_scale,
                                    chart::AmplitudeUnit::DezibelFullScale => db_full_scale,
                                };

                            let sample_rate = impulse_response.sample_rate as f32;

                            Chart::new()
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .x_range(
                                    self.chart_data
                                        .x_range
                                        .as_ref()
                                        .map(|r| {
                                            x_scale_fn(*r.start(), sample_rate)
                                                ..=x_scale_fn(*r.end(), sample_rate)
                                        })
                                        .unwrap_or_else(|| {
                                            x_scale_fn(-sample_rate / 2.0, sample_rate)
                                                ..=x_scale_fn(
                                                    impulse_response.data.len() as f32,
                                                    sample_rate,
                                                )
                                        }),
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
                                )
                                .on_scroll(|state: &pliced::chart::State<()>| {
                                    let pos = state.get_coords();
                                    let delta = state.scroll_delta();
                                    let x_range = state.x_range();
                                    Message::Chart(ChartOperation::Scroll(pos, delta, x_range))
                                })
                        };

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

    pub(crate) fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            keyboard::on_key_press(|key, _modifiers| match key {
                keyboard::Key::Named(keyboard::key::Named::Shift) => {
                    Some(Message::Chart(ChartOperation::ShiftKeyPressed))
                }
                _ => None,
            }),
            keyboard::on_key_release(|key, _modifiers| match key {
                keyboard::Key::Named(keyboard::key::Named::Shift) => {
                    Some(Message::Chart(ChartOperation::ShiftKeyReleased))
                }
                _ => None,
            }),
        ])
    }
}

impl ChartData {
    fn apply(&mut self, operation: ChartOperation) {
        match operation {
            ChartOperation::TimeUnitChanged(time_unit) => self.time_unit = time_unit,
            ChartOperation::AmplitudeUnitChanged(amplitude_unit) => {
                self.amplitude_unit = amplitude_unit
            }
            ChartOperation::Scroll(pos, scroll_delta, x_range) => {
                let Some(pos) = pos else {
                    return;
                };

                let Some(ScrollDelta::Lines { y, .. }) = scroll_delta else {
                    return;
                };

                if self.x_range.is_none() {
                    self.x_max = x_range.as_ref().map(|r| *r.end());
                    self.x_range = x_range;
                }

                match (self.shift_key_pressed, y.is_sign_positive()) {
                    (true, true) => self.scroll_right(),
                    (true, false) => self.scroll_left(),
                    (false, true) => self.zoom_in(pos),
                    (false, false) => self.zoom_out(pos),
                }
            }
            ChartOperation::ShiftKeyPressed => {
                self.shift_key_pressed = true;
            }
            ChartOperation::ShiftKeyReleased => {
                self.shift_key_pressed = false;
            }
        }
    }

    fn scroll_right(&mut self) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };

        let length = old_viewport.end() - old_viewport.start();

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = length * SCROLL_FACTOR;

        let mut new_end = old_viewport.end() + offset;
        if let Some(x_max) = self.x_max {
            let viewport_max = x_max + length / 2.0;
            if new_end > viewport_max {
                new_end = viewport_max;
            }
        }

        let new_start = new_end - length;

        self.x_range = Some(new_start..=new_end);
    }

    fn scroll_left(&mut self) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };
        let length = old_viewport.end() - old_viewport.start();

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = length * SCROLL_FACTOR;

        let mut new_start = old_viewport.start() - offset;
        let viewport_min = -(length / 2.0);
        if new_start < viewport_min {
            new_start = viewport_min;
        }
        let new_end = new_start + length;

        self.x_range = Some(new_start..=new_end);
    }

    fn zoom_in(&mut self, position: iced::Point) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };

        let old_len = old_viewport.end() - old_viewport.start();

        let center_scale: f32 = (position.x - old_viewport.start()) / old_len;

        // FIXME make configurable
        const ZOOM_FACTOR: f32 = 0.8;
        const LOWER_BOUND: f32 = 50.0;
        let mut new_len = old_len * ZOOM_FACTOR;
        if new_len < LOWER_BOUND {
            new_len = LOWER_BOUND;
        }

        let new_start = position.x - (new_len * center_scale);
        let new_end = new_start + new_len;
        self.x_range = Some(new_start..=new_end);
    }

    fn zoom_out(&mut self, position: iced::Point) {
        let Some(old_viewport) = self.x_range.clone() else {
            return;
        };
        let old_len = old_viewport.end() - old_viewport.start();

        let center_scale = (position.x - old_viewport.start()) / old_len;

        // FIXME make configurable
        const ZOOM_FACTOR: f32 = 1.2;
        let new_len = old_len * ZOOM_FACTOR;
        //if new_len >= self.max_len {
        //    new_len = self.max_len;
        //}

        let new_start = position.x - (new_len * center_scale);
        let new_end = new_start + new_len;
        self.x_range = Some(new_start..=new_end);
    }
}

fn percent_full_scale(s: f32, max: f32) -> f32 {
    s / max * 100f32
}

fn db_full_scale(s: f32, max: f32) -> f32 {
    let y = 20f32 * f32::log10(s.abs() / max);
    y.clamp(-80.0, max)
}

fn sample_scale(index: f32, _sample_rate: f32) -> f32 {
    index
}

fn time_scale(index: f32, sample_rate: f32) -> f32 {
    index / sample_rate * 1000.0
}
