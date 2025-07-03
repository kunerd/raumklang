use raumklang_core::dbfs;

use prism::{axis, line_series, Axis, Chart, Labels};

use iced::{
    keyboard,
    mouse::ScrollDelta,
    widget::{canvas, column, container, horizontal_space, pick_list, row, stack, text, toggler},
    Alignment, Color, Element,
    Length::{self, FillPortion},
    Point, Subscription,
};
use rand::Rng;
use rustfft::num_complex::Complex;

use crate::{
    data::{self, frequency_response, measurement},
    widgets::colored_circle,
};

use std::{fmt, ops::RangeInclusive};

#[derive(Debug, Clone)]
pub enum Message {
    ShowInGraphToggled(usize, bool),
    Chart(ChartOperation),
    SmoothingChanged(Smoothing),
}

pub enum Action {
    None,
    Smooth(Option<u8>),
}

pub struct FrequencyResponses {
    chart: ChartData,
    entries: Vec<Entry>,
    smoothing: Smoothing,
}

struct Entry {
    measurement_id: usize,
    show: bool,
    color: iced::Color,
}

#[derive(Default)]
pub struct ChartData {
    x_max: Option<f32>,
    x_range: Option<RangeInclusive<f32>>,
    cache: canvas::Cache,
    shift_key_pressed: bool,
}

#[derive(Debug, Clone)]
pub enum ChartOperation {
    Scroll(
        Option<Point>,
        Option<ScrollDelta>,
        Option<RangeInclusive<f32>>,
    ),
    ShiftKeyPressed,
    ShiftKeyReleased,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Smoothing {
    None,
    OneOne,
    OneSecond,
    OneThird,
    OneSixth,
    OneTwelfth,
    OneTwentyFourth,
    OneFourtyEighth,
}

impl FrequencyResponses {
    pub fn new(iter: impl Iterator<Item = usize>) -> Self {
        let entries = iter.map(Entry::new).collect();

        Self {
            chart: ChartData::default(),
            smoothing: Smoothing::None,
            entries,
        }
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::ShowInGraphToggled(id, state) => {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.show = state;
                }

                Action::None
            }
            Message::Chart(operation) => {
                self.chart.apply(operation);
                Action::None
            }
            Message::SmoothingChanged(smoothing) => {
                self.smoothing = smoothing;

                Action::Smooth(smoothing.fraction())
            }
        }
    }

    pub fn view<'a>(&'a self, measurements: &[&'a data::Measurement]) -> Element<'a, Message> {
        let sidebar = {
            let entries = self.entries.iter().enumerate().flat_map(
                |(id, entry)| -> Option<Element<Message>> {
                    let measurement = measurements.get(entry.measurement_id)?;

                    let name = &measurement.details.name;
                    let state = &measurement.analysis;

                    entry.view(id, name, state).into()
                },
            );

            container(column(entries).spacing(10).padding(8)).style(container::rounded_box)
        };

        let header = {
            let computed_frs = self
                .entries
                .iter()
                .flat_map(|entry| measurements.get(entry.measurement_id))
                .flat_map(|m| m.frequency_response())
                .count();

            let smoothing_options = if computed_frs == self.entries.len() {
                Some(pick_list(
                    Smoothing::ALL,
                    Some(&self.smoothing),
                    Message::SmoothingChanged,
                ))
            } else {
                None
            };

            row![].push_maybe(smoothing_options)
        };

        let content: Element<_> = if self.entries.iter().any(|entry| entry.show) {
            let series_list = self
                .entries
                .iter()
                .flat_map(|entry| {
                    if entry.show {
                        let measurement = measurements.get(entry.measurement_id)?;
                        let frequency_response = measurement.frequency_response()?;

                        Some((frequency_response, entry.color))
                    } else {
                        None
                    }
                })
                .flat_map(|(frequency_response, color)| {
                    let sample_rate = frequency_response.origin.sample_rate;
                    let len = frequency_response.origin.data.len() * 2 + 1;
                    let resolution = sample_rate as f32 / len as f32;

                    let closure = move |(i, s): (usize, &Complex<f32>)| {
                        (i as f32 * resolution, dbfs(s.re.abs()))
                    };
                    [
                        Some(
                            line_series(
                                frequency_response
                                    .origin
                                    .data
                                    .iter()
                                    .enumerate()
                                    .skip(1)
                                    .map(closure),
                            )
                            .color(color.scale_alpha(0.1)),
                        ),
                        frequency_response.smoothed.as_ref().map(|smoothed| {
                            { line_series(smoothed.iter().enumerate().skip(1).map(closure)) }
                                .color(color)
                        }),
                    ]
                })
                .flatten();

            let chart: Chart<Message, ()> = Chart::new()
                .x_axis(
                    Axis::new(axis::Alignment::Horizontal)
                        .scale(axis::Scale::Log)
                        .x_tick_marks(
                            [0, 20, 50, 100, 1000, 10_000, 20_000]
                                .into_iter()
                                .map(|v| v as f32)
                                .collect(),
                        ),
                )
                .x_range(self.chart.x_range.clone().unwrap_or(20.0..=22_500.0))
                .y_labels(Labels::default().format(&|v| format!("{v:.0}")))
                .extend_series(series_list)
                .cache(&self.chart.cache)
                .on_scroll(|state| {
                    let pos = state.get_coords();
                    let delta = state.scroll_delta();
                    let x_range = state.x_range();
                    Message::Chart(ChartOperation::Scroll(pos, delta, x_range))
                });

            chart.into()
        } else {
            text("Please select a frequency respone.").into()
        };

        row![
            container(sidebar)
                .width(FillPortion(1))
                .style(container::bordered_box),
            column![header, container(content).center(Length::FillPortion(4))].spacing(12)
        ]
        .spacing(10)
        .into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
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

    pub(crate) fn clear_cache(&self) {
        self.chart.cache.clear();
    }
}

impl Entry {
    pub fn new(id: usize) -> Self {
        Self {
            measurement_id: id,
            show: true,
            color: random_color(),
        }
    }

    pub fn view<'a>(
        &'a self,
        id: usize,
        name: &'a str,
        state: &'a measurement::Analysis,
    ) -> Element<'a, Message> {
        let entry = {
            let content = column![
                text(name).wrapping(text::Wrapping::Glyph),
                row![
                    toggler(self.show)
                        .on_toggle(move |state| Message::ShowInGraphToggled(id, state))
                        .width(Length::Shrink),
                    horizontal_space(),
                    colored_circle(10.0, self.color),
                ]
                .align_y(Alignment::Center)
            ]
            .clip(true)
            .spacing(5)
            .padding(5);

            container(content).style(container::rounded_box)
        };

        match state {
            measurement::Analysis::None => panic!(),
            measurement::Analysis::ImpulseResponse(_) => {
                processing_overlay("Impulse Response", entry).into()
            }
            measurement::Analysis::FrequencyResponse(_, frequency_response::State::Computing) => {
                processing_overlay("Frequency Response", entry).into()
            }
            measurement::Analysis::FrequencyResponse(_, frequency_response::State::Computed(_)) => {
                entry.into()
            }
        }
    }
}

fn random_color() -> iced::Color {
    const MAX_COLOR_VALUE: u8 = 255;

    // TODO: replace with color palette
    let red = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let green = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let blue = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);

    iced::Color::from_rgb8(red, green, blue)
}

fn processing_overlay<'a>(
    status: &'a str,
    entry: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
    stack([
        container(entry).style(container::bordered_box).into(),
        container(column![text("Computing..."), text(status).size(12)])
            .center(Length::Fill)
            .style(|theme| container::Style {
                border: container::rounded_box(theme).border,
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.8,
                ))),
                ..Default::default()
            })
            .into(),
    ])
    .into()
}

impl ChartData {
    fn apply(&mut self, operation: ChartOperation) {
        match operation {
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

                self.cache.clear();
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

impl Smoothing {
    const ALL: [Smoothing; 8] = [
        Smoothing::None,
        Smoothing::OneOne,
        Smoothing::OneSecond,
        Smoothing::OneThird,
        Smoothing::OneSixth,
        Smoothing::OneTwelfth,
        Smoothing::OneTwentyFourth,
        Smoothing::OneFourtyEighth,
    ];

    pub fn fraction(&self) -> Option<u8> {
        match self {
            Smoothing::None => None,
            Smoothing::OneOne => Some(1),
            Smoothing::OneSecond => Some(2),
            Smoothing::OneThird => Some(3),
            Smoothing::OneSixth => Some(6),
            Smoothing::OneTwelfth => Some(12),
            Smoothing::OneTwentyFourth => Some(24),
            Smoothing::OneFourtyEighth => Some(48),
        }
    }
}

impl fmt::Display for Smoothing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} smoothing",
            match self {
                Smoothing::None => "No",
                Smoothing::OneOne => "1/1",
                Smoothing::OneSecond => "1/2",
                Smoothing::OneThird => "1/3",
                Smoothing::OneSixth => "1/6",
                Smoothing::OneTwelfth => "1/12",
                Smoothing::OneTwentyFourth => "1/24",
                Smoothing::OneFourtyEighth => "1/48",
            }
        )
    }
}
