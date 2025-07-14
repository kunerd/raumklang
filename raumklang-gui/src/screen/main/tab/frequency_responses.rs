use raumklang_core::dbfs;

use prism::{axis, line_series, Axis, Chart, Labels};

use iced::{
    keyboard,
    mouse::ScrollDelta,
    widget::{canvas, column, container, horizontal_space, pick_list, row, stack, text, toggler},
    Alignment, Color, Element,
    Length::{self, FillPortion},
    Point, Subscription, Task,
};
use rand::Rng;
use rustfft::num_complex::{Complex, Complex32};

use crate::{
    data::{self, frequency_response, impulse_response, measurement},
    log,
    widgets::colored_circle,
};

use std::{collections::HashMap, fmt, ops::RangeInclusive};

#[derive(Debug, Clone)]
pub enum Message {
    ShowInGraphToggled(measurement::Id, bool),
    Chart(ChartOperation),
    SmoothingChanged(Smoothing),
    FrequencyResponseComputed((measurement::Id, data::FrequencyResponse)),
    ImpulseResponseComputed(Result<(measurement::Id, data::ImpulseResponse), data::Error>),
    FrequencyResponseSmoothed((measurement::Id, Box<[Complex<f32>]>)),
}

pub enum Action {
    None,
    Task(Task<Message>),
    ImpulseResponseComputed(measurement::Id, data::ImpulseResponse, Task<Message>),
}

#[derive(Debug, Default)]
pub struct FrequencyResponses {
    chart: ChartData,
    entries: HashMap<measurement::Id, Entry>,
    smoothing: Smoothing,
}

#[derive(Debug)]
struct Entry {
    show: bool,
    color: iced::Color,
    state: frequency_response::State,
    smoothed: Option<Box<[Complex<f32>]>>,
}

#[derive(Debug, Default)]
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

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Smoothing {
    #[default]
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
    pub fn new() -> Self {
        Self {
            chart: ChartData::default(),
            entries: HashMap::new(),
            smoothing: Smoothing::None,
        }
    }

    #[must_use]
    pub fn update(&mut self, message: Message, project: &data::Project) -> Action {
        match message {
            Message::ShowInGraphToggled(id, state) => {
                if let Some(entry) = self.entries.get_mut(&id) {
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

                if let Some(fraction) = smoothing.fraction() {
                    let tasks = self
                        .entries
                        .iter()
                        .flat_map(|(id, fr)| match &fr.state {
                            frequency_response::State::Computing => None,
                            frequency_response::State::Computed(frequency_response) => {
                                Some((*id, frequency_response.clone()))
                            }
                        })
                        .map(|(id, frequency_response)| {
                            Task::perform(
                                smooth_frequency_response(id, frequency_response, fraction),
                                Message::FrequencyResponseSmoothed,
                            )
                        });

                    Action::Task(Task::batch(tasks))
                } else {
                    self.entries
                        .values_mut()
                        .for_each(|entry| entry.smoothed = None);

                    self.chart.cache.clear();

                    Action::None
                }
            }
            Message::FrequencyResponseComputed((id, frequency_response)) => {
                if let Some(entry) = self.entries.get_mut(&id) {
                    entry.state = frequency_response::State::Computed(frequency_response.clone());
                    self.chart.cache.clear();

                    if let Some(fraction) = self.smoothing.fraction() {
                        Action::Task(Task::perform(
                            smooth_frequency_response(id, frequency_response, fraction),
                            Message::FrequencyResponseSmoothed,
                        ))
                    } else {
                        Action::None
                    }
                } else {
                    Action::None
                }
            }
            Message::ImpulseResponseComputed(Ok((id, impulse_response))) => {
                let computation = frequency_response::Computation::from_impulse_response(
                    id,
                    impulse_response.clone(),
                    project.window().clone(),
                );

                let task = Task::perform(computation.run(), Message::FrequencyResponseComputed);

                Action::ImpulseResponseComputed(id, impulse_response, task)
            }
            Message::ImpulseResponseComputed(Err(err)) => {
                log::error!("{}", err);
                Action::None
            }
            Message::FrequencyResponseSmoothed((id, smoothed_data)) => {
                if let Some(frequency_response) = self.entries.get_mut(&id) {
                    frequency_response.smoothed = Some(smoothed_data);
                    self.chart.cache.clear();
                }

                Action::None
            }
        }
    }

    pub(crate) fn refresh<'a>(
        &mut self,
        project: &'a data::Project,
        impulse_response: &'a HashMap<measurement::Id, impulse_response::State>,
    ) -> Task<Message> {
        let mut tasks = vec![];

        for measurement in project.measurements.loaded() {
            self.entries.entry(measurement.id).or_insert(Entry::new());

            if let Some(impulse_response::State::Computed(impulse_response)) =
                impulse_response.get(&measurement.id)
            {
                let computation = frequency_response::Computation::from_impulse_response(
                    measurement.id,
                    impulse_response.clone(),
                    project.window().clone(),
                );

                tasks.push(Task::perform(
                    computation.run(),
                    Message::FrequencyResponseComputed,
                ));
            } else {
                let computation = project
                    .impulse_response_computation(measurement.id)
                    .unwrap();

                tasks.push(Task::perform(
                    computation.run(),
                    Message::ImpulseResponseComputed,
                ));
            }
        }

        Task::batch(tasks)
    }

    pub fn view<'a>(&'a self, measurements: &'a measurement::List) -> Element<'a, Message> {
        let sidebar = {
            let entries = measurements.loaded().flat_map(|measurement| {
                if let Some(entry) = self.entries.get(&measurement.id) {
                    let name = &measurement.details.name;

                    entry.view(measurement.id, name).into()
                } else {
                    None
                }
            });

            container(column(entries).spacing(10).padding(8)).style(container::rounded_box)
        };

        let header = {
            row![pick_list(
                Smoothing::ALL,
                Some(&self.smoothing),
                Message::SmoothingChanged,
            )]
        };

        let content: Element<_> = if self.entries.values().any(|entry| entry.show) {
            let series_list = self
                .entries
                .values()
                .filter(|entry| entry.show)
                .map(|entry| {
                    let frequency_response::State::Computed(frequency_response) = &entry.state
                    else {
                        return [None, None];
                    };

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
                            .color(entry.color.scale_alpha(0.1)),
                        ),
                        entry.smoothed.as_ref().map(|smoothed| {
                            { line_series(smoothed.iter().enumerate().skip(1).map(closure)) }
                                .color(entry.color)
                        }),
                    ]
                })
                .flatten()
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

    pub(crate) fn remove(&mut self, id: measurement::Id) {
        self.entries.remove(&id);
    }
}

impl Entry {
    pub fn new() -> Self {
        Self {
            show: true,
            color: random_color(),
            state: frequency_response::State::Computing,
            smoothed: None,
        }
    }

    pub fn view<'a>(&'a self, id: measurement::Id, name: &'a str) -> Element<'a, Message> {
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

        match &self.state {
            frequency_response::State::Computing => processing_overlay("Frequency Response", entry),
            frequency_response::State::Computed(_frequency_response) => entry.into(),
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

async fn smooth_frequency_response(
    id: measurement::Id,
    frequency_response: data::FrequencyResponse,
    fraction: u8,
) -> (measurement::Id, Box<[Complex<f32>]>) {
    let data: Vec<_> = frequency_response
        .origin
        .data
        .iter()
        .map(|c| c.re.abs())
        .collect();

    let data: Vec<_> = tokio::task::spawn_blocking(move || {
        frequency_response::smooth_fractional_octave(&data, fraction)
    })
    .await
    .unwrap()
    .into_iter()
    .map(Complex32::from)
    .collect();

    (id, data.into_boxed_slice())
}
