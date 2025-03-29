use crate::data::{
    self,
    chart::{self},
    impulse_response,
    window::{self},
    Samples,
};

use pliced::chart::{line_series, point_series, Chart, Labels, PointStyle};

use iced::{
    keyboard,
    mouse::ScrollDelta,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, pick_list, row, scrollable,
        text,
    },
    Alignment, Element, Length, Point, Subscription,
};

use core::panic;
use std::{ops::RangeInclusive, time::Duration};

pub struct ImpulseReponses {
    window_settings: WindowSettings,
    selected: Option<usize>,
    chart_data: ChartData,
}

struct WindowSettings {
    window: data::Window<data::Samples>,
    hovered: Option<usize>,
    dragging: Dragging,
}

#[derive(Debug, Clone)]
pub enum Message {
    Select(usize),
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
    ComputeImpulseResponse(usize),
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
            window_settings,
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
            Message::Window(operation) => {
                self.window_settings
                    .apply(operation, self.chart_data.time_unit);

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
                                .on_scroll(|state: &pliced::chart::State<SeriesId>| {
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
                                            PointStyle {
                                                color: Some(iced::Color::from_rgb8(127, 127, 127)),
                                                radius: 10.0,
                                                ..Default::default()
                                            }
                                        } else {
                                            PointStyle::default()
                                        }
                                    })
                                    .color(iced::Color::from_rgb8(255, 0, 0)),
                                )
                                .on_press(|state: &pliced::chart::State<SeriesId>| {
                                    let id = state.items().and_then(|l| l.first().map(|i| i.1));
                                    Message::Window(WindowOperation::MouseDown(
                                        id,
                                        state.get_offset(),
                                    ))
                                })
                                .on_move(|state: &pliced::chart::State<SeriesId>| {
                                    let id = state.items().and_then(|l| l.first().map(|i| i.1));
                                    Message::Window(WindowOperation::OnMove(id, state.get_offset()))
                                })
                                .on_release(|state: &pliced::chart::State<SeriesId>| {
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

impl WindowSettings {
    pub fn apply(&mut self, operation: WindowOperation, time_unit: chart::TimeSeriesUnit) {
        let mut update_handle = |id, prev_pos: iced::Point, pos: iced::Point| {
            let offset = pos.x - prev_pos.x;

            let mut handles: window::Handles = Into::into(&self.window);
            match id {
                0 => handles.move_left(offset),
                1 => handles.move_center(offset),
                2 => handles.move_right(offset),
                n => panic!("there should be no handles with index: {n}"),
            }

            match time_unit {
                chart::TimeSeriesUnit::Time => {
                    let mut window: data::Window<Duration> = self.window.clone().into();
                    window.update(handles);
                    self.window = window.into();
                }

                chart::TimeSeriesUnit::Samples => self.window.update(handles),
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
                            // let pos = update_handle(id, prev_pos, pos);
                            update_handle(id, prev_pos, pos);
                            self.dragging = Dragging::ForSure(id, pos);
                        }
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        // let pos = update_handle(id, prev_pos, pos);
                        update_handle(id, prev_pos, pos);
                        self.dragging = Dragging::ForSure(id, pos);
                    }
                    Dragging::None => {
                        if id.is_none() {
                            // if let Some(handle) =
                            //     self.hovered.and_then(|id| self.handles.get_mut(id))
                            // {
                            //     handle.style = PointStyle::default();
                            // }
                        }
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
                        // if let Some(handle) = self.handles.get_mut(id) {
                        //     handle.style = PointStyle::default();
                        // }
                        self.hovered = None;
                        self.dragging = Dragging::None;
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        update_handle(id, prev_pos, pos);
                        // if let Some(handle) = self.handles.get_mut(id) {
                        //     handle.style = PointStyle::default();
                        // }
                        self.dragging = Dragging::None;
                    }
                    Dragging::None => {}
                }
            }
        }
    }
}

// impl Window {
//     pub fn new(window: data::Window, sample_rate: u32) -> Self {
//         // let max_size = max_size as f32;
//         // let half_size = max_size as f32 / 2.0;

//         // let sample_rate = sample_rate as f32;
//         // let left_window_size =
//         //     (sample_rate * (Self::DEFAULT_LEFT_DURATION / 1000.0)).min(half_size);
//         // let right_window_size =
//         //     (sample_rate * (Self::DEFAULT_RIGTH_DURATION / 1000.0)).min(half_size);

//         // let left_side_left = 0.0;
//         // let left_side_right = left_side_left + left_window_size as f32;
//         // let right_side_left = left_side_right;
//         // let right_side_right = right_side_left + right_window_size as f32;

//         // let handles = vec![
//         //     WindowHandle::new(left_side_left, 0.0),
//         //     WindowHandle::new(left_side_right, 1.0),
//         //     WindowHandle::new(right_side_left, 1.0),
//         //     WindowHandle::new(right_side_right, 0.0),
//         // ];

//         // Self {
//         //     max_size,
//         //     sample_rate,
//         //     handles,
//         //     dragging: Dragging::None,
//         //     hovered_item: None,
//         // }
//         //
//         Self {}
//     }

//     fn apply(&mut self, operation: WindowOperation, time_unit: chart::TimeSeriesUnit) {
//         let mut update_handle_pos =
//             |id: usize, prev_pos: iced::Point, pos: iced::Point| -> iced::Point {
//                 let min = match id {
//                     0 => f32::MIN,
//                     id => self.handles[id - 1].x,
//                 };

//                 let max = if let Some(handle) = self.handles.get(id + 1) {
//                     handle.x
//                 } else {
//                     self.max_size
//                 };

//                 let Some(handle) = self.handles.get_mut(id) else {
//                     return prev_pos;
//                 };

//                 let offset = prev_pos.x - pos.x;
//                 let offset = match time_unit {
//                     chart::TimeSeriesUnit::Time => offset / 1000.0 * self.sample_rate,
//                     chart::TimeSeriesUnit::Samples => offset,
//                 };

//                 let new_pos = handle.x - offset;

//                 if new_pos >= min {
//                     if new_pos <= max {
//                         handle.x = new_pos;
//                         pos
//                     } else {
//                         let mut x_clamped = handle.x - max;
//                         if matches!(time_unit, chart::TimeSeriesUnit::Time) {
//                             x_clamped *= 1000.0 / self.sample_rate;
//                         }
//                         x_clamped = prev_pos.x - x_clamped;

//                         handle.x = max;

//                         iced::Point::new(x_clamped, pos.y)
//                     }
//                 } else {
//                     let mut x_clamped = handle.x - min;
//                     if matches!(time_unit, chart::TimeSeriesUnit::Time) {
//                         x_clamped *= 1000.0 / self.sample_rate;
//                     }
//                     x_clamped = prev_pos.x - x_clamped;

//                     handle.x = min;

//                     iced::Point::new(x_clamped, pos.y)
//                 }
//             };

//         match operation {
//             WindowOperation::MouseDown(id, pos) => {
//                 let Dragging::None = self.dragging else {
//                     return;
//                 };

//                 if let (Some(id), Some(pos)) = (id, pos) {
//                     self.dragging = Dragging::CouldStillBeClick(id, pos);
//                 }
//             }
//             WindowOperation::OnMove(id, pos) => {
//                 let Some(pos) = pos else {
//                     return;
//                 };

//                 match self.dragging {
//                     Dragging::CouldStillBeClick(id, prev_pos) => {
//                         if prev_pos != pos {
//                             let pos = update_handle_pos(id, prev_pos, pos);
//                             self.dragging = Dragging::ForSure(id, pos);
//                         }
//                     }
//                     Dragging::ForSure(id, prev_pos) => {
//                         let pos = update_handle_pos(id, prev_pos, pos);
//                         self.dragging = Dragging::ForSure(id, pos);
//                     }
//                     Dragging::None => {
//                         if id.is_none() {
//                             if let Some(handle) =
//                                 self.hovered_item.and_then(|id| self.handles.get_mut(id))
//                             {
//                                 handle.style = PointStyle::default();
//                             }
//                         }
//                         self.hovered_item = id;
//                     }
//                 }
//             }
//             WindowOperation::MouseUp(pos) => {
//                 let Some(pos) = pos else {
//                     return;
//                 };

//                 match self.dragging {
//                     Dragging::CouldStillBeClick(id, _point) => {
//                         if let Some(handle) = self.handles.get_mut(id) {
//                             handle.style = PointStyle::default();
//                         }
//                         self.hovered_item = None;
//                         self.dragging = Dragging::None;
//                     }
//                     Dragging::ForSure(id, prev_pos) => {
//                         update_handle_pos(id, prev_pos, pos);
//                         if let Some(handle) = self.handles.get_mut(id) {
//                             handle.style = PointStyle::default();
//                         }
//                         self.dragging = Dragging::None;
//                     }
//                     Dragging::None => {}
//                 }
//             }
//         }

//         let yellow: iced::Color = iced::Color::from_rgb8(238, 230, 0);
//         let green: iced::Color = iced::Color::from_rgb8(50, 205, 50);

//         match self.dragging {
//             Dragging::CouldStillBeClick(id, _point) | Dragging::ForSure(id, _point) => {
//                 if let Some(handle) = self.handles.get_mut(id) {
//                     handle.style = PointStyle {
//                         color: Some(green),
//                         radius: 10.0,
//                         ..Default::default()
//                     }
//                 }
//             }
//             Dragging::None => {
//                 if let Some(handle) = self.hovered_item.and_then(|id| self.handles.get_mut(id)) {
//                     handle.style = PointStyle {
//                         color: Some(yellow),
//                         radius: 8.0,
//                         ..Default::default()
//                     }
//                 }
//             }
//         }
//     }

//     fn curve(&self) -> impl Iterator<Item = (f32, f32)> + Clone {
//         let left_side = raumklang_core::Window::Hann;
//         let right_side = raumklang_core::Window::Hann;

//         let left_side_width = (self.handles[1].x - self.handles[0].x).round() as usize;
//         let offset = (self.handles[2].x - self.handles[1].x).round() as usize;
//         let right_side_width = (self.handles[3].x - self.handles[2].x).round() as usize;
//         let window: Vec<_> =
//             WindowBuilder::new(left_side, left_side_width, right_side, right_side_width)
//                 .set_offset(offset)
//                 .build()
//                 .into_iter()
//                 .enumerate()
//                 .map(|(x, y)| (x as f32 + self.handles[0].x, y))
//                 .collect();

//         window.into_iter()
//     }
// }

// impl WindowHandle {
//     pub fn new(x: f32, y: f32) -> Self {
//         Self {
//             x,
//             y,
//             style: PointStyle::default(),
//         }
//     }
// }

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
