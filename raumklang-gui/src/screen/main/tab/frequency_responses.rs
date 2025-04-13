use raumklang_core::dbfs;

use pliced::chart::{line_series, Chart, Labels};

use iced::{
    keyboard,
    mouse::ScrollDelta,
    widget::{column, container, horizontal_space, row, stack, text, toggler},
    Alignment, Color, Element,
    Length::{self, FillPortion},
    Point, Subscription,
};
use rand::Rng;

use crate::{
    data::{self, frequency_response, measurement},
    widgets::colored_circle,
};

use std::ops::RangeInclusive;

pub struct FrequencyResponses {
    chart: ChartData,
    entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ShowInGraphToggled(usize, bool),
    Chart(ChartOperation),
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

impl FrequencyResponses {
    pub fn new(iter: impl Iterator<Item = usize>) -> Self {
        let entries = iter.map(Entry::new).collect();

        Self {
            chart: ChartData::default(),
            entries,
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
                .map(|(frequency_response, color)| {
                    let sample_reate = frequency_response.origin.sample_rate;
                    let len = frequency_response.origin.data.len() * 2;
                    let resolution = sample_reate as f32 / len as f32;

                    line_series(
                        frequency_response
                            .origin
                            .data
                            .iter()
                            .enumerate()
                            .map(move |(i, s)| (i as f32 * resolution, dbfs(s.re.abs()))),
                    )
                    .color(color)
                });

            let length = self
                .entries
                .iter()
                .find_map(|entry| {
                    if entry.show {
                        let measurement = measurements.get(entry.measurement_id)?;
                        let frequency_response = measurement.frequency_response()?;

                        Some(frequency_response)
                    } else {
                        None
                    }
                })
                .map_or(0.0..=22_050.0, |fr| 0.0..=fr.origin.data.len() as f32);

            let chart: Chart<Message, ()> = Chart::new()
                .x_range(self.chart.x_range.clone().unwrap_or(length))
                .y_labels(Labels::default().format(&|v| format!("{v:.0}")))
                .extend_series(series_list)
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
            container(content).center(Length::FillPortion(4))
        ]
        .into()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ShowInGraphToggled(id, state) => {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.show = state;
                }
            }
            Message::Chart(operation) => self.chart.apply(operation),
        }
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
