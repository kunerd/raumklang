use super::chart;

use crate::{
    data::{self, window},
    ui::ImpulseResponse,
};

use iced::{
    mouse::ScrollDelta,
    widget::{canvas, column, container, pick_list, row, stack, text},
    Alignment, Color, Element, Length, Point,
};
use prism::{items, line_series, point_series, series::point, Items, Labels};

use std::{ops::RangeInclusive, time::Duration};

#[derive(Debug, Clone)]
pub enum Message {
    Chart(ChartOperation),
    // Chart(chart::Interaction),
    Window(WindowOperation),
}

#[derive(Debug, Clone)]
pub enum ChartOperation {
    TimeUnitChanged(data::chart::TimeSeriesUnit),
    AmplitudeUnitChanged(data::chart::AmplitudeUnit),
    Scroll(
        Option<Point>,
        Option<ScrollDelta>,
        Option<RangeInclusive<f32>>,
    ),
    Interaction(chart::Interaction),
}

#[derive(Debug, Default)]
pub struct Chart {
    pub x_max: Option<f32>,
    pub x_range: Option<RangeInclusive<f32>>,
    shift_key_pressed: bool,
    pub amplitude_unit: data::chart::AmplitudeUnit,
    pub time_unit: data::chart::TimeSeriesUnit,
    pub cache: canvas::Cache,
    pub line_cache: canvas::Cache,
}

impl Chart {
    pub(crate) fn update(&mut self, chart_operation: ChartOperation) {
        match chart_operation {
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
                    self.x_range = x_range;
                }

                match (self.shift_key_pressed, y.is_sign_positive()) {
                    (true, true) => self.scroll_right(),
                    (true, false) => self.scroll_left(),
                    (false, true) => self.zoom_in(pos),
                    (false, false) => self.zoom_out(pos),
                }
            }
            ChartOperation::Interaction(chart::Interaction::HandleMoved(index, distance)) => {}
        }

        self.cache.clear();
        self.line_cache.clear();
    }

    pub(crate) fn view<'a>(
        &'a self,
        impulse_response: &'a ImpulseResponse,
        window_settings: &'a WindowSettings,
    ) -> Element<'a, Message> {
        let header = {
            pick_list(
                &data::chart::AmplitudeUnit::ALL[..],
                Some(&self.amplitude_unit),
                |unit| Message::Chart(ChartOperation::AmplitudeUnitChanged(unit)),
            )
        };

        let chart = {
            container(
                chart::impulse_response(
                    &window_settings.window,
                    impulse_response,
                    &self.time_unit,
                    &self.amplitude_unit,
                    &self.line_cache,
                )
                .map(ChartOperation::Interaction)
                .map(Message::Chart),
            )
            .style(container::rounded_box)
        };
        // let chart = {
        //     let x_scale_fn = match self.time_unit {
        //         chart::TimeSeriesUnit::Samples => sample_scale,
        //         chart::TimeSeriesUnit::Time => time_scale,
        //     };

        //     let y_scale_fn: fn(f32, f32) -> f32 = match self.amplitude_unit {
        //         chart::AmplitudeUnit::PercentFullScale => percent_full_scale,
        //         chart::AmplitudeUnit::DezibelFullScale => db_full_scale,
        //     };

        //     let sample_rate = impulse_response.sample_rate.into();

        //     let chart = prism::Chart::new()
        //         .width(Length::Fill)
        //         .height(Length::Fill)
        //         .cache(&self.cache)
        //         .x_range(
        //             self.x_range
        //                 .as_ref()
        //                 .map(|r| {
        //                     x_scale_fn(*r.start(), sample_rate)..=x_scale_fn(*r.end(), sample_rate)
        //                 })
        //                 .unwrap_or_else(|| {
        //                     x_scale_fn(-sample_rate / 2.0, sample_rate)
        //                         ..=x_scale_fn(impulse_response.data.len() as f32, sample_rate)
        //                 }),
        //         )
        //         .x_labels(Labels::default().format(&|v| format!("{v:.2}")))
        //         .y_labels(Labels::default().format(&|v| format!("{v:.2}")))
        //         .push_series(
        //             line_series(impulse_response.data.iter().enumerate().map(move |(i, s)| {
        //                 (
        //                     x_scale_fn(i as f32, sample_rate),
        //                     y_scale_fn(*s, impulse_response.max),
        //                 )
        //             }))
        //             .cache(&self.line_cache)
        //             .color(iced::Color::from_rgb8(2, 125, 66)),
        //         )
        //         .on_scroll(|state| {
        //             let pos = state.get_coords();
        //             let delta = state.scroll_delta();
        //             let x_range = state.x_range();
        //             Message::Chart(ChartOperation::Scroll(pos, delta, x_range))
        //         });

        //     // chart
        //     let window_curve = window_settings.window.curve();
        //     let handles: window::Handles = (&window_settings.window).into();

        //     chart
        //         .push_series(
        //             line_series(
        //                 window_curve
        //                     .map(move |(i, s)| (x_scale_fn(i, sample_rate), y_scale_fn(s, 1.0))),
        //             )
        //             // .cache(&window_settings.cache)
        //             .color(iced::Color::from_rgb8(255, 0, 0)),
        //         )
        //         .push_series(
        //             point_series(handles.into_iter().map(move |handle| {
        //                 (
        //                     x_scale_fn(handle.x(), sample_rate),
        //                     y_scale_fn(handle.y().into(), 1.0),
        //                 )
        //             }))
        //             .style_for_each(|index, _handle| {
        //                 if window_settings.hovered.is_some_and(|i| i == index) {
        //                     point::Style {
        //                         color: Some(iced::Color::from_rgb8(220, 250, 250)),
        //                         radius: 10.0,
        //                         ..Default::default()
        //                     }
        //                 } else {
        //                     point::Style::default()
        //                 }
        //             })
        //             .cache(&window_settings.cache)
        //             .color(iced::Color::from_rgb8(255, 0, 0)),
        //         )
        //         .items(&window_settings.items)
        //         .on_press(|state| {
        //             let id = state.items().and_then(|l| l.first().copied());
        //             Message::Window(WindowOperation::MouseDown(id, state.get_offset()))
        //         })
        //         .on_move(|state| {
        //             let id = state.items().and_then(|l| l.first().copied());
        //             Message::Window(WindowOperation::OnMove(id, state.get_offset()))
        //         })
        //         .on_release(|state| Message::Window(WindowOperation::MouseUp(state.get_offset())))
        // };

        let footer = {
            row![container(pick_list(
                &data::chart::TimeSeriesUnit::ALL[..],
                Some(&self.time_unit),
                |unit| Message::Chart(ChartOperation::TimeUnitChanged(unit))
            ))
            .align_right(Length::Fill)]
            .align_y(Alignment::Center)
        };

        container(column![header, chart, footer].spacing(8)).into()
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

    pub(crate) fn shift_key_released(&mut self) {
        self.shift_key_pressed = false;
    }

    pub(crate) fn shift_key_pressed(&mut self) {
        self.shift_key_pressed = true
    }
}

#[derive(Debug)]
pub struct WindowSettings {
    pub window: data::Window<data::Samples>,
    hovered: Option<usize>,
    dragging: Dragging,
    items: prism::Items,
    is_dirty: bool,
    pub cache: canvas::Cache,
}

#[derive(Debug, Default)]
pub enum Dragging {
    CouldStillBeClick(usize, iced::Point),
    ForSure(usize, iced::Point),
    #[default]
    None,
}

#[derive(Debug, Clone)]
pub enum WindowOperation {
    OnMove(Option<usize>, Option<iced::Point>),
    MouseDown(Option<usize>, Option<iced::Point>),
    MouseUp(Option<iced::Point>),
}

impl WindowSettings {
    pub(crate) fn new(window: data::Window<data::Samples>) -> Self {
        let time_unit = data::chart::TimeSeriesUnit::default();
        let x_scale_fn = match time_unit {
            data::chart::TimeSeriesUnit::Samples => sample_scale,
            data::chart::TimeSeriesUnit::Time => time_scale,
        };

        let amplitude_unit = data::chart::AmplitudeUnit::default();
        let y_scale_fn: fn(f32, f32) -> f32 = match amplitude_unit {
            data::chart::AmplitudeUnit::PercentFullScale => percent_full_scale,
            data::chart::AmplitudeUnit::DezibelFullScale => db_full_scale,
        };

        let sample_rate = window.sample_rate();
        let handles: window::Handles = (&window).into();
        let mut items = Items::new(iced::Size::new(10.0, 10.0));

        items.add_series(handles.into_iter().enumerate().map(|(id, handle)| {
            items::Entry::new(
                id,
                (
                    x_scale_fn(handle.x(), sample_rate.into()),
                    y_scale_fn(handle.y().into(), 1.0),
                ),
            )
        }));

        Self {
            window,
            hovered: None,
            dragging: Dragging::None,
            items,
            cache: canvas::Cache::new(),
            is_dirty: false,
        }
    }

    pub fn apply(
        &mut self,
        operation: WindowOperation,
        time_unit: data::chart::TimeSeriesUnit,
        amplitude_unit: data::chart::AmplitudeUnit,
    ) {
        let mut update_handle = |id, prev_pos: iced::Point, pos: iced::Point| {
            let offset = pos.x - prev_pos.x;

            match time_unit {
                data::chart::TimeSeriesUnit::Time => {
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

                data::chart::TimeSeriesUnit::Samples => {
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

            let x_scale_fn = match time_unit {
                data::chart::TimeSeriesUnit::Samples => sample_scale,
                data::chart::TimeSeriesUnit::Time => time_scale,
            };

            let y_scale_fn: fn(f32, f32) -> f32 = match amplitude_unit {
                data::chart::AmplitudeUnit::PercentFullScale => percent_full_scale,
                data::chart::AmplitudeUnit::DezibelFullScale => db_full_scale,
            };

            let sample_rate = self.window.sample_rate();
            let handles: window::Handles = (&self.window).into();
            let mut items = Items::new(iced::Size::new(10.0, 10.0));

            items.add_series(handles.into_iter().enumerate().map(|(id, handle)| {
                items::Entry::new(
                    id,
                    (
                        x_scale_fn(handle.x(), sample_rate.into()),
                        y_scale_fn(handle.y().into(), 1.0),
                    ),
                )
            }));

            self.items = items;
            self.cache.clear();
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
                        self.cache.clear();
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
                        self.cache.clear();
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        update_handle(id, prev_pos, pos);
                        self.dragging = Dragging::None;
                    }
                    Dragging::None => {}
                }
            }
        }
        self.is_dirty = true;
    }

    pub fn is_dirty(&self) -> bool {
        self.is_dirty
    }
}

pub fn processing_overlay<'a, Message>(
    status: &'a str,
    entry: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
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
