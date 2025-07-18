use crate::{
    data::{self, chart},
    ui::{self, ImpulseResponse},
};

use iced::{
    mouse::ScrollDelta,
    widget::{
        button, canvas, column, container, horizontal_rule, pick_list, row, scrollable, stack, text,
    },
    Alignment, Color, Element, Length, Point,
};
use prism::{line_series, point_series, Labels};

use std::{collections::HashMap, ops::RangeInclusive};

#[derive(Debug, Default)]
pub struct Tab {
    selected: Option<ui::measurement::Id>,
    chart: Chart,
}

impl Tab {}

#[derive(Debug, Clone)]
pub enum ChartOperation {
    TimeUnitChanged(chart::TimeSeriesUnit),
    AmplitudeUnitChanged(chart::AmplitudeUnit),
    Scroll(
        Option<Point>,
        Option<ScrollDelta>,
        Option<RangeInclusive<f32>>,
    ),
    // ShiftKeyPressed,
    // ShiftKeyReleased,
}

#[derive(Debug, Default)]
pub struct Chart {
    x_max: Option<f32>,
    x_range: Option<RangeInclusive<f32>>,
    shift_key_pressed: bool,
    amplitude_unit: chart::AmplitudeUnit,
    time_unit: chart::TimeSeriesUnit,
    cache: canvas::Cache,
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
                    self.x_max = x_range.as_ref().map(|r| *r.end());
                    self.x_range = x_range;
                }

                match (self.shift_key_pressed, y.is_sign_positive()) {
                    (true, true) => self.scroll_right(),
                    (true, false) => self.scroll_left(),
                    (false, true) => self.zoom_in(pos),
                    (false, false) => self.zoom_out(pos),
                }
            } // ChartOperation::ShiftKeyPressed => {
              //     self.shift_key_pressed = true;
              // }
              // ChartOperation::ShiftKeyReleased => {
              //     self.shift_key_pressed = false;
              // }
        }

        self.cache.clear();
    }

    pub(crate) fn view<'a>(
        &'a self,
        impulse_response: &'a ImpulseResponse,
    ) -> Element<'a, ChartOperation> {
        let header = {
            pick_list(
                &chart::AmplitudeUnit::ALL[..],
                Some(&self.amplitude_unit),
                ChartOperation::AmplitudeUnitChanged,
            )
        };

        let chart = {
            let x_scale_fn = match self.time_unit {
                chart::TimeSeriesUnit::Samples => sample_scale,
                chart::TimeSeriesUnit::Time => time_scale,
            };

            let y_scale_fn: fn(f32, f32) -> f32 = match self.amplitude_unit {
                chart::AmplitudeUnit::PercentFullScale => percent_full_scale,
                chart::AmplitudeUnit::DezibelFullScale => db_full_scale,
            };

            let sample_rate = impulse_response.sample_rate as f32;

            let chart: prism::Chart<'_, ChartOperation, ()> = prism::Chart::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .cache(&self.cache)
                .x_range(
                    self.x_range
                        .as_ref()
                        .map(|r| {
                            x_scale_fn(*r.start(), sample_rate)..=x_scale_fn(*r.end(), sample_rate)
                        })
                        .unwrap_or_else(|| {
                            x_scale_fn(-sample_rate / 2.0, sample_rate)
                                ..=x_scale_fn(impulse_response.data.len() as f32, sample_rate)
                        }),
                )
                .x_labels(Labels::default().format(&|v| format!("{v:.2}")))
                .y_labels(Labels::default().format(&|v| format!("{v:.2}")))
                .push_series(
                    line_series(impulse_response.data.iter().enumerate().map(move |(i, s)| {
                        (
                            x_scale_fn(i as f32, sample_rate),
                            y_scale_fn(*s, impulse_response.max),
                        )
                    }))
                    .color(iced::Color::from_rgb8(2, 125, 66)),
                )
                .on_scroll(|state| {
                    let pos = state.get_coords();
                    let delta = state.scroll_delta();
                    let x_range = state.x_range();
                    ChartOperation::Scroll(pos, delta, x_range)
                });

            chart
            // let window_curve = self.window_settings.window.curve();
            // let handles: window::Handles = Into::into(&self.window_settings.window);
            // chart.push_series(
            //     line_series(
            //         window_curve
            //             .map(move |(i, s)| (x_scale_fn(i, sample_rate), y_scale_fn(s, 1.0))),
            //     )
            //     .color(iced::Color::from_rgb8(255, 0, 0)),
            // )
            // .push_series(
            //     point_series(handles.into_iter().map(move |handle| {
            //         (
            //             x_scale_fn(handle.x(), sample_rate),
            //             y_scale_fn(handle.y().into(), 1.0),
            //         )
            //     }))
            //     .with_id(SeriesId::Handles)
            //     .style_for_each(|index, _handle| {
            //         if self.window_settings.hovered.is_some_and(|i| i == index) {
            //             point::Style {
            //                 color: Some(iced::Color::from_rgb8(220, 250, 250)),
            //                 radius: 10.0,
            //                 ..Default::default()
            //             }
            //         } else {
            //             point::Style::default()
            //         }
            //     })
            //     .color(iced::Color::from_rgb8(255, 0, 0)),
            // )
            // .on_press(|state| {
            //     let id = state.items().and_then(|l| l.first().map(|i| i.1));
            //     Message::Window(WindowOperation::MouseDown(id, state.get_offset()))
            // })
            // .on_move(|state| {
            //     let id = state.items().and_then(|l| l.first().map(|i| i.1));
            //     Message::Window(WindowOperation::OnMove(id, state.get_offset()))
            // })
            // .on_release(|state| Message::Window(WindowOperation::MouseUp(state.get_offset())))
        };

        let footer = {
            row![container(pick_list(
                &chart::TimeSeriesUnit::ALL[..],
                Some(&self.time_unit),
                ChartOperation::TimeUnitChanged
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
        self.shift_key_pressed = true;
    }

    pub(crate) fn shift_key_pressed(&mut self) {
        self.shift_key_pressed = false
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
