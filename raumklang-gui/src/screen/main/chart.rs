pub mod spectrogram;
pub mod waveform;

use waveform::Waveform;

use crate::{
    data::{self, chart, window::Handles, Samples, Window},
    screen::main::chart::spectrogram::Spectrogram,
    ui,
};

use iced::{
    advanced::{
        graphics::text::Paragraph,
        text::{self, Paragraph as _},
    },
    alignment,
    mouse::{self, ScrollDelta},
    widget::{
        canvas::{self, Frame, Path, Stroke},
        container,
        text::{Fragment, IntoFragment},
    },
    window, Element, Event, Font,
    Length::Fill,
    Pixels, Point, Rectangle, Renderer, Size, Theme, Vector,
};

use std::{
    cmp::Ordering,
    ops::{Add, RangeInclusive, Sub},
};

pub fn waveform<'a>(
    measurement: &'a raumklang_core::Measurement,
    cache: &'a canvas::Cache,
    zoom: Zoom,
    offset: Offset,
) -> Element<'a, waveform::Interaction, iced::Theme> {
    canvas::Canvas::new(Waveform {
        datapoints: measurement.iter().copied(),
        cache,
        cmp: |a, b| a.total_cmp(b),
        y_to_float: |s| s,
        to_x_scale: move |i| i as f32,
        // to_x_scale: move |i| match time_unit {
        //     chart::TimeSeriesUnit::Time => time_scale(i, impulse_response.sample_rate.into()),
        //     chart::TimeSeriesUnit::Samples => i as f32,
        // },
        // to_y_scale: move |s| match amplitude_unit {
        //     chart::AmplitudeUnit::PercentFullScale => percent_full_scale(s),
        //     chart::AmplitudeUnit::DezibelFullScale => db_full_scale(s),
        // },
        zoom,
        offset,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

pub fn spectrogram<'a>(
    data: &'a data::Spectrogram,
    cache: &'a canvas::Cache,
    zoom: Zoom,
    offset: Offset,
) -> Element<'a, spectrogram::Interaction, iced::Theme> {
    canvas::Canvas::new(Spectrogram {
        datapoints: data,
        cache,
        // cmp: |a, b| a.total_cmp(b),
        // y_to_float: |s| s,
        // to_x_scale: move |i| i as f32,
        // to_x_scale: move |i| match time_unit {
        //     chart::TimeSeriesUnit::Time => time_scale(i, impulse_response.sample_rate.into()),
        //     chart::TimeSeriesUnit::Samples => i as f32,
        // },
        // to_y_scale: move |s| match amplitude_unit {
        //     chart::AmplitudeUnit::PercentFullScale => percent_full_scale(s),
        //     chart::AmplitudeUnit::DezibelFullScale => db_full_scale(s),
        // },
        zoom,
        offset,
    })
    .width(Fill)
    .height(Fill)
    .into()
}

pub fn impulse_response<'a>(
    window: &'a Window<Samples>,
    impulse_response: &'a ui::ImpulseResponse,
    time_unit: &'a chart::TimeSeriesUnit,
    amplitude_unit: &'a chart::AmplitudeUnit,
    zoom: Zoom,
    offset: i64,
    data_cache: &'a canvas::Cache,
    overlay_cache: &'a canvas::Cache,
) -> Element<'a, Interaction, iced::Theme> {
    container(
        canvas::Canvas::new(BarChart {
            window,
            datapoints: impulse_response
                .data
                .iter()
                .copied()
                .map(f32::abs)
                .enumerate(),
            cmp: |a, b| a.total_cmp(b),
            to_x_scale: move |i| match time_unit {
                chart::TimeSeriesUnit::Time => time_scale(i, impulse_response.sample_rate.into()),
                chart::TimeSeriesUnit::Samples => i as f32,
            },
            y_to_float: |s| s,
            to_y_scale: move |s| match amplitude_unit {
                chart::AmplitudeUnit::PercentFullScale => percent_full_scale(s),
                chart::AmplitudeUnit::DezibelFullScale => db_full_scale(s),
            },
            zoom,
            offset,
            data_cache,
            overlay_cache,
        })
        .width(Fill)
        .height(Fill),
    )
    .into()
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Zoom(f32);

impl Default for Zoom {
    fn default() -> Self {
        Zoom(1.0)
    }
}

impl Add<f32> for Zoom {
    type Output = Zoom;

    fn add(self, rhs: f32) -> Self::Output {
        Zoom(self.0 + rhs)
    }
}

impl Sub<f32> for Zoom {
    type Output = Zoom;

    fn sub(self, rhs: f32) -> Self::Output {
        Zoom(self.0 - rhs)
    }
}

impl From<Zoom> for f32 {
    fn from(zoom: Zoom) -> Self {
        zoom.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Offset(isize);
impl Offset {
    fn saturating_add(&self, rhs: isize) -> Offset {
        Offset(self.0.saturating_add(rhs))
    }

    fn saturating_sub(&self, rhs: isize) -> Offset {
        Offset(self.0.saturating_sub(rhs))
    }
}

impl Default for Offset {
    fn default() -> Self {
        Self(0)
    }
}

impl From<Offset> for f32 {
    fn from(value: Offset) -> Self {
        value.0 as f32
    }
}

impl From<Offset> for isize {
    fn from(value: Offset) -> Self {
        value.0
    }
}

struct BarChart<'a, I, X, Y, ScaleX, ScaleY>
where
    I: Iterator<Item = (X, Y)>,
    // ToFloat: Fn(Y) -> f32,
{
    window: &'a Window<Samples>,
    datapoints: I,
    cmp: fn(&Y, &Y) -> Ordering,
    to_x_scale: ScaleX,
    y_to_float: fn(Y) -> f32,
    to_y_scale: ScaleY,
    zoom: Zoom,
    offset: i64,
    data_cache: &'a canvas::Cache,
    overlay_cache: &'a canvas::Cache,
}

#[derive(Debug, Clone)]
pub enum Interaction {
    HandleMoved(usize, f32),
    ZoomChanged(Zoom),
    OffsetChanged(i64),
}

#[derive(Default)]
enum State<'a> {
    #[default]
    Initalizing,
    Initialized {
        plane: Rectangle,
        x_axis: HorizontalAxis<'a>,
        y_axis: VerticalAxis<'a>,
        hovered_handle: Option<usize>,
        dragging: Dragging,
        shift_pressed: bool,
    },
}

#[derive(Debug, Clone, Copy, Default)]
enum Dragging {
    CouldStillBeClick(usize, iced::Point),
    ForSure(usize, iced::Point),
    #[default]
    None,
}

impl<'a, I, X, Y, ScaleX, ScaleY> canvas::Program<Interaction, iced::Theme>
    for BarChart<'a, I, X, Y, ScaleX, ScaleY>
where
    I: Iterator<Item = (X, Y)> + Clone + 'a,
    Y: Copy + std::iter::Sum,
    ScaleX: Fn(f32) -> f32,
    ScaleY: Fn(f32) -> f32,
{
    type State = State<'static>;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Interaction>> {
        if let Event::Window(window::Event::RedrawRequested(_)) = event {
            let x_min = 0.250 * 44_100 as f32 * f32::from(self.zoom);
            let x_min = -x_min + self.offset as f32;

            let x_max = 0.6 * 44_100 as f32 * f32::from(self.zoom);
            let x_max = x_max.ceil() as u64;

            let datapoints = self
                .datapoints
                .clone()
                .skip(if x_min > 0.0 {
                    x_min.ceil() as usize
                } else {
                    0
                })
                .take(x_max.saturating_add_signed(self.offset) as usize);

            let datapoints = datapoints.clone().map(|(_i, datapoint)| datapoint);

            let Some(min) = datapoints.clone().min_by(self.cmp) else {
                return None;
            };

            let Some(max) = datapoints.clone().max_by(self.cmp) else {
                return None;
            };

            let min_value = (self.to_y_scale)((self.y_to_float)(min));
            let max_value = (self.to_y_scale)((self.y_to_float)(max)) + 10.0;

            let x_max = datapoints.clone().count() as f32;

            let x_range = x_min..=x_max;
            let x_axis = HorizontalAxis::new(x_range, &self.to_x_scale, 10);

            let y_range = min_value..=max_value;
            let y_axis = VerticalAxis::new(y_range, 10);

            let plane = Rectangle::new(
                Point::new(bounds.x + y_axis.width, bounds.y),
                Size::new(bounds.width - y_axis.width, bounds.height - x_axis.height),
            );

            *state = match state {
                State::Initalizing => State::Initialized {
                    plane,
                    x_axis,
                    y_axis,
                    hovered_handle: None,
                    dragging: Dragging::None,
                    shift_pressed: false,
                },

                State::Initialized {
                    hovered_handle,
                    dragging,
                    shift_pressed,
                    ..
                } => State::Initialized {
                    plane,
                    x_axis,
                    y_axis,
                    hovered_handle: *hovered_handle,
                    dragging: *dragging,
                    shift_pressed: *shift_pressed,
                },
            }
        }

        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. }) => {
                let State::Initialized {
                    x_axis,
                    y_axis,
                    plane,
                    ref mut hovered_handle,
                    ref mut dragging,
                    ..
                } = state
                else {
                    return None;
                };

                let cursor = cursor.position_from(plane.position())?;
                let pixels_per_unit_x = plane.width / x_axis.length;

                match dragging {
                    Dragging::CouldStillBeClick(id, prev_pos) => {
                        if *prev_pos != cursor {
                            let distance = (cursor.x - prev_pos.x) / pixels_per_unit_x;
                            let new_pos = (prev_pos.x / pixels_per_unit_x) + distance + x_axis.min;

                            let action = Some(canvas::Action::publish(Interaction::HandleMoved(
                                *id, new_pos,
                            )));

                            *dragging = Dragging::ForSure(*id, cursor);
                            self.overlay_cache.clear();

                            action
                        } else {
                            None
                        }
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        let distance = (cursor.x - prev_pos.x) / pixels_per_unit_x;
                        let new_pos = (prev_pos.x / pixels_per_unit_x) + distance + x_axis.min;

                        let action = Some(canvas::Action::publish(Interaction::HandleMoved(
                            *id, new_pos,
                        )));

                        *dragging = Dragging::ForSure(*id, cursor);
                        self.overlay_cache.clear();

                        action
                    }
                    Dragging::None => {
                        let radius = 5.0;
                        let handles = Handles::from(self.window);

                        let y_target_length = plane.height - y_axis.min_label_height * 0.5;
                        let pixels_per_unit = y_target_length / y_axis.length;

                        let x_min = -x_axis.min;
                        let hovered = handles.iter().enumerate().find_map(|(i, handle)| {
                            let y = match handle.y() {
                                crate::data::window::handle::Alignment::Bottom => 0.0,
                                crate::data::window::handle::Alignment::Center => 0.5,
                                crate::data::window::handle::Alignment::Top => 1.0,
                            };

                            let y = (self.to_y_scale)(y);

                            let bounding_box = Rectangle::new(
                                Point {
                                    x: x_min * pixels_per_unit_x + handle.x() * pixels_per_unit_x
                                        - radius,
                                    y: plane.height - (y - y_axis.min) * pixels_per_unit - radius,
                                },
                                iced::Size {
                                    width: 2.0 * radius,
                                    height: 2.0 * radius,
                                },
                            );

                            if bounding_box.contains(cursor) {
                                Some(i)
                            } else {
                                None
                            }
                        });

                        if *hovered_handle != hovered {
                            *hovered_handle = hovered;
                            self.overlay_cache.clear();

                            Some(canvas::Action::request_redraw())
                        } else {
                            None
                        }
                    }
                }
            }
            Event::Mouse(mouse::Event::ButtonPressed(mouse::Button::Left)) => {
                let State::Initialized {
                    hovered_handle,
                    plane,
                    ref mut dragging,
                    ..
                } = state
                else {
                    return None;
                };

                let Dragging::None = dragging else {
                    return None;
                };

                let hovered = (*hovered_handle)?;
                let cursor = cursor.position_from(plane.position())?;

                self.overlay_cache.clear();

                *dragging = Dragging::CouldStillBeClick(hovered, cursor);

                None
            }
            Event::Mouse(mouse::Event::ButtonReleased(mouse::Button::Left)) => {
                let State::Initialized {
                    x_axis,
                    plane,
                    ref mut hovered_handle,
                    ref mut dragging,
                    ..
                } = state
                else {
                    return None;
                };

                let Some(cursor) = cursor.position_from(plane.position()) else {
                    *dragging = Dragging::None;
                    *hovered_handle = None;

                    self.overlay_cache.clear();

                    return Some(canvas::Action::request_redraw());
                };

                match dragging {
                    Dragging::CouldStillBeClick(_id, _point) => {
                        *dragging = Dragging::None;

                        None
                    }
                    Dragging::ForSure(id, prev_pos) => {
                        let pixels_per_unit_x = plane.width / x_axis.length;

                        let distance = (cursor.x - prev_pos.x) / pixels_per_unit_x;
                        let new_pos = (prev_pos.x / pixels_per_unit_x) + distance + x_axis.min;

                        let action = Some(canvas::Action::publish(Interaction::HandleMoved(
                            *id, new_pos,
                        )));

                        *dragging = Dragging::None;
                        *hovered_handle = None;

                        self.overlay_cache.clear();

                        action
                    }
                    Dragging::None => None,
                }
            }
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                let State::Initialized { shift_pressed, .. } = state else {
                    return None;
                };

                if let iced::keyboard::Key::Named(iced::keyboard::key::Named::Shift) = key {
                    *shift_pressed = true;
                }

                None
            }
            Event::Keyboard(iced::keyboard::Event::KeyReleased { key, .. }) => {
                let State::Initialized { shift_pressed, .. } = state else {
                    return None;
                };

                if let iced::keyboard::Key::Named(iced::keyboard::key::Named::Shift) = key {
                    *shift_pressed = false;
                }

                None
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let State::Initialized { shift_pressed, .. } = state else {
                    return None;
                };

                let ScrollDelta::Lines { y, .. } = delta else {
                    return None;
                };

                if *shift_pressed {
                    let new_offset = if y.is_sign_positive() {
                        self.offset + (0.05 * f32::from(self.zoom) * 44_100_f32).ceil() as i64
                    } else {
                        self.offset - (0.05 * f32::from(self.zoom) * 44_100_f32).ceil() as i64
                    };

                    if self.offset != new_offset {
                        self.data_cache.clear();
                        self.overlay_cache.clear();

                        Some(canvas::Action::publish(Interaction::OffsetChanged(
                            new_offset,
                        )))
                    } else {
                        None
                    }
                } else {
                    let new_zoom = if y.is_sign_positive() {
                        self.zoom - (self.zoom.0 * 0.1)
                    } else {
                        self.zoom + (self.zoom.0 * 0.1)
                    };

                    self.data_cache.clear();
                    self.overlay_cache.clear();

                    Some(canvas::Action::publish(Interaction::ZoomChanged(new_zoom)))
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        state: &State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let palette = theme.extended_palette();

        let State::Initialized {
            x_axis,
            y_axis,
            hovered_handle,
            plane,
            ..
        } = state
        else {
            return vec![];
        };

        let pixels_per_unit_x = plane.width / x_axis.length;
        let window_size = if pixels_per_unit_x < 1.0 {
            Some((x_axis.length / plane.width).floor() as usize)
        } else {
            None
        };

        let bar_width = if window_size.is_some() {
            1.0
        } else {
            pixels_per_unit_x
        };

        let y_target_length = plane.height - y_axis.min_label_height * 0.5;
        let pixels_per_unit = y_target_length / y_axis.length;

        let data = self.data_cache.draw(renderer, bounds.size(), |frame| {
            let x_min = 0.250 * 44_100 as f32 * f32::from(self.zoom);
            let x_min = -x_min + self.offset as f32;

            let x_max = 0.6 * 44_100 as f32 * f32::from(self.zoom);
            let x_max = x_max.ceil() as u64;

            let datapoints = self
                .datapoints
                .clone()
                .skip(if x_min > 0.0 {
                    x_min.ceil() as usize
                } else {
                    0
                })
                .take(x_max.saturating_add_signed(self.offset) as usize)
                .map(|(_i, datapoint)| datapoint);

            let x_min = -x_axis.min;
            for (i, datapoint) in datapoints.enumerate() {
                let value = datapoint;

                let value = (self.y_to_float)(value);
                let value = (self.to_y_scale)(value);

                let bar_height = (value - y_axis.min) * pixels_per_unit;

                let bar = Rectangle {
                    x: y_axis.width + (x_min * pixels_per_unit_x) + (i as f32 * pixels_per_unit_x),
                    y: plane.height - bar_height,
                    width: bar_width,
                    height: bar_height,
                };

                frame.fill_rectangle(bar.position(), bar.size(), palette.secondary.weak.color);
            }

            frame.with_save(|frame| {
                frame.translate(Vector::new(y_axis.width, 0.0));

                x_axis.draw(frame, plane.width);
            });

            y_axis.draw(frame, y_target_length);
        });

        let overlay = self.overlay_cache.draw(renderer, bounds.size(), |frame| {
            let x_min = -x_axis.min;
            let mut window_curve = self.window.curve().map(|(x, y)| (x, (self.to_y_scale)(y)));

            let path = Path::new(|b| {
                if let Some((x, y)) = window_curve.next() {
                    b.move_to(Point {
                        x: y_axis.width + x_min * pixels_per_unit_x + x * pixels_per_unit_x,
                        y: plane.height - (y - y_axis.min) * pixels_per_unit,
                    });
                    window_curve.fold(b, |acc, (x, y)| {
                        acc.line_to(Point {
                            x: y_axis.width + x_min * pixels_per_unit_x + x * pixels_per_unit_x,
                            y: plane.height - (y - y_axis.min) * pixels_per_unit,
                        });
                        acc
                    });
                }
            });

            frame.stroke(
                &path,
                Stroke::default()
                    .with_width(2.0)
                    .with_color(palette.success.weak.color),
            );

            let radius = 5.0;
            let handles = Handles::from(self.window);
            for (i, handle) in handles.iter().enumerate() {
                let y = match handle.y() {
                    crate::data::window::handle::Alignment::Bottom => 0.0,
                    crate::data::window::handle::Alignment::Center => 0.5,
                    crate::data::window::handle::Alignment::Top => 1.0,
                };

                let y = (self.to_y_scale)(y);

                let center = Point {
                    x: y_axis.width + x_min * pixels_per_unit_x + handle.x() * pixels_per_unit_x,
                    y: plane.height - (y - y_axis.min) * pixels_per_unit,
                };

                let path = Path::circle(center, radius);
                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_color(if hovered_handle.is_some_and(|selected| i == selected) {
                            palette.primary.strong.color
                        } else {
                            palette.secondary.strong.color
                        })
                        .with_width(2.0),
                );
            }

            // if let Some(cursor) = cursor.position() {
            //     let path = Path::line(
            //         Point {
            //             x: cursor.x,
            //             y: 0.0,
            //         },
            //         Point {
            //             x: cursor.x,
            //             y: bounds.height - x_axis.height,
            //         },
            //     );

            //     frame.stroke(
            //         &path,
            //         Stroke::default()
            //             .with_width(2.0)
            //             .with_color(palette.background.weakest.color),
            //     );
            // }
        });

        vec![data, overlay]
    }

    fn mouse_interaction(
        &self,
        _state: &Self::State,
        _bounds: Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> iced::advanced::mouse::Interaction {
        iced::advanced::mouse::Interaction::default()
    }
}

struct HorizontalAxis<'a> {
    min: f32,
    length: f32,
    height: f32,
    labels: Vec<Label<'a>>,
    scale: Scale,
    range: RangeInclusive<f32>,
}

#[derive(Default, Clone, Copy)]
pub enum Scale {
    #[default]
    Linear,
    Log,
}

impl<'a> HorizontalAxis<'a> {
    pub fn with_labels<F: Fn(f32) -> f32, I: IntoIterator<Item = f32>>(
        range: RangeInclusive<f32>,
        to_scale: F,
        labels: I,
    ) -> Self {
        let length = range.end() - range.start();

        let labels: Vec<_> = labels
            .into_iter()
            .map(|t| {
                let l = (to_scale)(t);
                Label::new(t, format!("{:.0}", l), 12.0)
            })
            .collect();

        let min_label_height = labels
            .iter()
            .map(|l| l.min_height())
            .min_by(f32::total_cmp)
            .unwrap();

        Self {
            min: *range.start(),
            length,
            height: min_label_height,
            labels,
            scale: Scale::default(),
            range,
        }
    }

    pub fn new<F: Fn(f32) -> f32>(
        range: RangeInclusive<f32>,
        to_scale: F,
        tick_amount: usize,
    ) -> Self {
        let length = range.end() - range.start();
        let tick_distance = length / tick_amount as f32;

        let min = *range.start();
        let offset = -min % tick_distance;
        let labels = (0..=tick_amount)
            .into_iter()
            .map(|t| offset + min + t as f32 * tick_distance);

        Self::with_labels(range, to_scale, labels)
    }

    pub fn scale(mut self, scale: Scale) -> Self {
        self.scale = scale;
        self
    }

    pub fn draw(&self, frame: &mut Frame, target_length: f32) {
        let pixels_per_unit = target_length / self.length;

        for label in self.labels.iter() {
            let value = label.value - self.min;

            // if !self.range.contains(&value) {
            //     continue;
            // }

            let value = if let Scale::Log = self.scale {
                self.log_scale(value)
            } else {
                value
            };

            let x = value * pixels_per_unit;
            let y = frame.height() - self.height;

            let position = Point::new(x, y);

            frame.fill_text(canvas::Text {
                content: label.content.to_string(),
                position,
                size: Pixels(12.0),
                color: iced::Color::WHITE,
                align_x: text::Alignment::Center,
                align_y: alignment::Vertical::Top,
                font: Font::MONOSPACE,
                ..canvas::Text::default()
            });
        }
    }

    fn log_scale(&self, value: f32) -> f32 {
        if value == 0.0 {
            0.0
        } else {
            (value.log10() / self.length.log10()) * self.length
        }
    }
}

struct VerticalAxis<'a> {
    min: f32,
    width: f32,
    length: f32,
    min_label_height: f32,
    labels: Vec<Label<'a>>,
}

impl<'a> VerticalAxis<'a> {
    pub fn new(range: RangeInclusive<f32>, tick_amount: usize) -> Self {
        let length = range.end() - range.start();

        let tick_distance = length / tick_amount as f32;

        let min = *range.start();
        let offset = -min % tick_distance;
        let labels = (0..=tick_amount)
            .into_iter()
            .map(|t| offset + min + t as f32 * tick_distance)
            .map(|l| Label::new(l, format!("{:.0}", l), 12.0));

        let min_label_width = labels
            .clone()
            .map(|l| l.min_width())
            .max_by(f32::total_cmp)
            .unwrap();

        let min_label_height = labels.clone().next().map(|l| l.min_height()).unwrap();

        Self {
            min: *range.start(),
            width: min_label_width, // + padding + thickness
            length,
            min_label_height,
            labels: labels.collect(),
        }
    }

    pub fn draw(&self, frame: &mut Frame, target_length: f32) {
        let pixels_per_unit = target_length / self.length;

        for label in self.labels.iter() {
            let y = (label.value - self.min) * pixels_per_unit;

            frame.fill_text(canvas::Text {
                content: label.content.to_string(),
                position: Point::new(
                    f32::from(self.width),
                    target_length + self.min_label_height * 0.5 - y,
                ),
                size: Pixels(12.0),
                color: iced::Color::WHITE,
                align_x: text::Alignment::Right,
                align_y: alignment::Vertical::Center,
                font: Font::MONOSPACE,
                ..canvas::Text::default()
            });
        }
    }
}

pub struct Label<'a> {
    value: f32,
    content: Fragment<'a>,
    bounds: iced::Size,
}

impl<'a> Label<'a> {
    pub fn new(value: f32, content: impl IntoFragment<'a>, font_size: impl Into<Pixels>) -> Self {
        let content = content.into_fragment();
        let bounds = min_bounds(content.as_ref(), font_size.into());

        Self {
            value,
            content,
            bounds,
        }
    }

    pub fn min_width(&self) -> f32 {
        self.bounds.width
    }

    pub fn min_height(&self) -> f32 {
        self.bounds.height
    }

    // pub fn draw<Renderer: geometry::Renderer>(
    //     &self,
    //     frame: &mut Frame<Renderer>,
    //     pos: iced::Point,
    //     alignment: Alignment,
    //     config: &Labels,
    // ) {
    //     let (align_x, align_y) = match alignment {
    //         Alignment::Horizontal => (text::Alignment::Center, alignment::Vertical::Top),
    //         Alignment::Vertical => (text::Alignment::Right, alignment::Vertical::Center),
    //     };

    //     frame.fill_text(canvas::Text {
    //         content: self.content.to_string(),
    //         size: config.font_size.unwrap_or(12.into()),
    //         position: pos,
    //         color: config.color.unwrap_or(iced::Color::WHITE),
    //         align_x,
    //         align_y,
    //         font: Font::MONOSPACE,
    //         ..canvas::Text::default()
    //     });
    // }
}

fn min_bounds(content: &str, font_size: Pixels) -> iced::Size {
    let text = iced::advanced::text::Text {
        content,
        size: font_size,
        line_height: text::LineHeight::default(),
        bounds: iced::Size::INFINITE,
        font: Font::MONOSPACE,
        align_x: iced::advanced::text::Alignment::Right,
        align_y: alignment::Vertical::Center,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::default(),
    };

    let paragraph = Paragraph::with_text(text);
    paragraph.min_bounds()
}

fn time_scale(index: f32, sample_rate: f32) -> f32 {
    index / sample_rate * 1000.0
}

fn percent_full_scale(s: f32) -> f32 {
    (s.abs() * 100f32).clamp(0.0, 100.0)
}

fn db_full_scale(s: f32) -> f32 {
    let y = 20f32 * f32::log10(s.abs());
    y.clamp(-80.0, 0.0)
}
