use crate::data::{
    self,
    chart::{self},
    impulse_response, measurement,
    window::{self},
    Samples,
};

use prism::{line_series, point_series, series::point, Chart, Labels};

use iced::{
    keyboard,
    mouse::ScrollDelta,
    widget::{
        button, canvas, column, container, horizontal_rule, horizontal_space, pick_list, row,
        scrollable, stack, text,
    },
    Alignment, Color, Element, Length, Point, Subscription,
};

use core::panic;
use std::{collections::HashMap, ops::RangeInclusive, time::Duration};

pub struct ImpulseReponses {
    selected: Option<measurement::Id>,
    window_settings: WindowSettings,
    chart_data: ChartData,
    pub items: HashMap<measurement::Id, impulse_response::State>,
}

struct WindowSettings {
    window: data::Window<data::Samples>,
    hovered: Option<usize>,
    dragging: Dragging,
}

#[derive(Debug, Clone)]
pub enum Message {
    Select(measurement::Id),
    Chart(ChartOperation),
    Window(WindowOperation),
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum SeriesId {
    Handles,
}

#[derive(Default)]
pub struct ChartData {
    x_max: Option<f32>,
    x_range: Option<RangeInclusive<f32>>,
    shift_key_pressed: bool,
    amplitude_unit: chart::AmplitudeUnit,
    time_unit: chart::TimeSeriesUnit,
    cache: canvas::Cache,
}

#[derive(Debug, Default)]
enum Dragging {
    CouldStillBeClick(usize, iced::Point),
    ForSure(usize, iced::Point),
    #[default]
    None,
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

#[derive(Debug, Clone)]
pub enum WindowOperation {
    OnMove(Option<usize>, Option<iced::Point>),
    MouseDown(Option<usize>, Option<iced::Point>),
    MouseUp(Option<iced::Point>),
}

pub enum Action {
    ComputeImpulseResponse(measurement::Id),
    WindowModified(data::Window<data::Samples>),
    None,
}

impl ImpulseReponses {
    pub fn new(window: &data::Window<Samples>) -> Self {
        let window_settings = WindowSettings {
            window: window.clone(),
            hovered: None,
            dragging: Dragging::None,
        };

        Self {
            selected: None,
            items: HashMap::new(),
            window_settings,
            chart_data: ChartData::default(),
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Select(id) => {
                self.selected = Some(id);

                if self.items.get(&id).is_none() {
                    self.items.insert(id, impulse_response::State::Computing);
                    Action::ComputeImpulseResponse(id)
                } else {
                    Action::None
                }
            }
            Message::Chart(operation) => {
                self.chart_data.apply(operation);

                Action::None
            }
            Message::Window(operation) => {
                self.window_settings
                    .apply(operation, self.chart_data.time_unit);

                self.chart_data.cache.clear();

                Action::WindowModified(self.window_settings.window.clone())
            }
        }
    }

    pub fn view<'a>(&'a self, measurements: &'a measurement::List) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                column!(text("For Measurements"), horizontal_rule(1))
                    .width(Length::Fill)
                    .spacing(5)
            };

            let entries = measurements
                .loaded()
                .map(|measurement| (measurement, self.items.get(&measurement.id)))
                .map(|(measurement, ir)| {
                    let id = measurement.id;

                    let entry = {
                        let content = column![text(&measurement.details.name).size(16),]
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
                    };

                    if let Some(ir) = ir {
                        match &ir {
                            impulse_response::State::Computing => {
                                processing_overlay("Impulse Response", entry)
                            }
                            impulse_response::State::Computed(_) => entry,
                        }
                    } else {
                        entry
                    }
                });

            container(scrollable(
                column![header, column(entries).spacing(3)]
                    .spacing(10)
                    .padding(10),
            ))
            .style(container::rounded_box)
        }
        .width(Length::FillPortion(1));

        let content: Element<_> = {
            if let Some(id) = self.selected {
                let state = self.items.get(&id).and_then(|ir| match &ir {
                    impulse_response::State::Computing => None,
                    impulse_response::State::Computed(ir) => Some(ir),
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

                        let chart = {
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

                            let chart = Chart::new()
                                .width(Length::Fill)
                                .height(Length::Fill)
                                .cache(&self.chart_data.cache)
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
                                .x_labels(Labels::default().format(&|v| format!("{v:.2}")))
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
                                .on_scroll(|state| {
                                    let pos = state.get_coords();
                                    let delta = state.scroll_delta();
                                    let x_range = state.x_range();
                                    Message::Chart(ChartOperation::Scroll(pos, delta, x_range))
                                });

                            let window_curve = self.window_settings.window.curve();
                            let handles: window::Handles = Into::into(&self.window_settings.window);
                            chart
                                .push_series(
                                    line_series(window_curve.map(move |(i, s)| {
                                        (x_scale_fn(i, sample_rate), y_scale_fn(s, 1.0))
                                    }))
                                    .color(iced::Color::from_rgb8(255, 0, 0)),
                                )
                                .push_series(
                                    point_series(handles.into_iter().map(move |handle| {
                                        (
                                            x_scale_fn(handle.x(), sample_rate),
                                            y_scale_fn(handle.y().into(), 1.0),
                                        )
                                    }))
                                    .with_id(SeriesId::Handles)
                                    .style_for_each(|index, _handle| {
                                        if self.window_settings.hovered.is_some_and(|i| i == index)
                                        {
                                            point::Style {
                                                color: Some(iced::Color::from_rgb8(220, 250, 250)),
                                                radius: 10.0,
                                                ..Default::default()
                                            }
                                        } else {
                                            point::Style::default()
                                        }
                                    })
                                    .color(iced::Color::from_rgb8(255, 0, 0)),
                                )
                                .on_press(|state| {
                                    let id = state.items().and_then(|l| l.first().map(|i| i.1));
                                    Message::Window(WindowOperation::MouseDown(
                                        id,
                                        state.get_offset(),
                                    ))
                                })
                                .on_move(|state| {
                                    let id = state.items().and_then(|l| l.first().map(|i| i.1));
                                    Message::Window(WindowOperation::OnMove(id, state.get_offset()))
                                })
                                .on_release(|state| {
                                    Message::Window(WindowOperation::MouseUp(state.get_offset()))
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

    pub(crate) fn remove(&mut self, id: measurement::Id) -> Option<impulse_response::State> {
        self.items.remove(&id)
    }
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

        self.cache.clear();
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

impl WindowSettings {
    pub fn apply(&mut self, operation: WindowOperation, time_unit: chart::TimeSeriesUnit) {
        let mut update_handle = |id, prev_pos: iced::Point, pos: iced::Point| {
            let offset = pos.x - prev_pos.x;

            match time_unit {
                chart::TimeSeriesUnit::Time => {
                    let mut window: data::Window<Duration> = self.window.clone().into();

                    let mut handles: window::Handles = Into::into(&window);
                    match id {
                        0 => handles.move_left(offset),
                        1 => handles.move_center(offset),
                        2 => handles.move_right(offset),
                        n => panic!("there should be no handles with index: {n}"),
                    }
                    window.update(handles);

                    self.window = window.into();
                }

                chart::TimeSeriesUnit::Samples => {
                    let mut handles: window::Handles = Into::into(&self.window);
                    match id {
                        0 => handles.move_left(offset),
                        1 => handles.move_center(offset),
                        2 => handles.move_right(offset),
                        n => panic!("there should be no handles with index: {n}"),
                    }

                    self.window.update(handles);
                }
            }
        };
        match operation {
            WindowOperation::MouseDown(id, pos) => {
                let Dragging::None = self.dragging else {
                    return;
                };

                if let (Some(id), Some(pos)) = (id, pos) {
                    self.dragging = Dragging::CouldStillBeClick(id, pos);
                }
            }
            WindowOperation::OnMove(id, pos) => {
                let Some(pos) = pos else {
                    return;
                };

                match self.dragging {
                    Dragging::CouldStillBeClick(id, prev_pos) => {
                        if prev_pos != pos {
                            update_handle(id, prev_pos, pos);
                            self.dragging = Dragging::ForSure(id, pos);
                        }
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        update_handle(id, prev_pos, pos);
                        self.dragging = Dragging::ForSure(id, pos);
                    }
                    Dragging::None => {
                        self.hovered = id;
                    }
                }
            }
            WindowOperation::MouseUp(pos) => {
                let Some(pos) = pos else {
                    return;
                };

                match self.dragging {
                    Dragging::CouldStillBeClick(_id, _point) => {
                        self.hovered = None;
                        self.dragging = Dragging::None;
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        update_handle(id, prev_pos, pos);
                        self.dragging = Dragging::None;
                    }
                    Dragging::None => {}
                }
            }
        }
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
