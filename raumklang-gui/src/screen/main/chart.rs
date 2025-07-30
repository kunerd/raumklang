use std::{cmp::Ordering, ops::RangeInclusive};

use iced::{
    advanced::{
        graphics::text::Paragraph,
        text::{self, Paragraph as _},
    },
    alignment, mouse,
    widget::{
        canvas::{self, Frame},
        container,
        text::{Fragment, IntoFragment},
    },
    Element, Event, Font,
    Length::Fill,
    Pixels, Point, Rectangle, Renderer, Theme, Vector,
};

use crate::ui;

pub fn impulse_response<'a>(
    impulse_response: &'a ui::ImpulseResponse,
    x_range: &'a RangeInclusive<f32>,
    cache: &'a canvas::Cache,
) -> Element<'a, Interaction, iced::Theme> {
    container(
        canvas::Canvas::new(BarChart {
            datapoints: impulse_response
                .data
                .iter()
                .copied()
                .map(|s| db_full_scale(s, impulse_response.max))
                .enumerate(),
            cache,
            cmp: |a, b| a.total_cmp(b),
            to_float: |s| s,
            // to_float: Box::new(|s| percent_full_scale(s, impulse_response.max) as f64),
            to_string: |s| format!("{s}:0."),
            x_range,
            // y_range: N,
            // x_range: 0..impulse_response.data.len(),
            // average: |duration, n| duration / n,
            // average_to_float: |duration| duration.as_secs_f64(),
            // average_to_string: |duration| format!("{duration:?}"),
        })
        .width(Fill)
        .height(Fill),
    )
    .padding(5)
    .into()
}

fn percent_full_scale(s: f32, max: f32) -> f32 {
    (s / max * 100f32).clamp(0.0, 100.0)
}

fn db_full_scale(s: f32, max: f32) -> f32 {
    let y = 20f32 * f32::log10(s.abs() / max);
    y.clamp(-80.0, max)
}

struct BarChart<'a, I, T>
where
    I: Iterator<Item = (usize, T)>,
{
    datapoints: I,
    cache: &'a canvas::Cache,
    cmp: fn(&T, &T) -> Ordering,
    to_float: fn(T) -> f32,
    to_string: fn(T) -> String,
    x_range: &'a RangeInclusive<f32>,
    // y_range: &'a RangeInclusive<f32>,
    // average: fn(T, u32) -> A,
    // average_to_float: fn(A) -> f64,
    // average_to_string: fn(A) -> String,
    // zoom: Zoom,
}

#[derive(Debug, Clone)]
pub enum Interaction {}

impl<'a, I, T> canvas::Program<Interaction, iced::Theme> for BarChart<'a, I, T>
where
    I: Iterator<Item = (usize, T)> + Clone + 'a,
    T: Copy + std::iter::Sum,
{
    type State = ();

    fn update(
        &self,
        // bar_hovered: &mut Option<timeline::Index>,
        _state: &mut Self::State,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Interaction>> {
        match event {
            // Event::Mouse(mouse::Event::CursorMoved { .. })
            // | Event::Window(window::Event::RedrawRequested(_)) => {
            //     let Some(position) = cursor.position_in(bounds) else {
            //         // if bar_hovered.is_some() {
            //         //     *bar_hovered = None;

            //         //     return Some(canvas::Action::publish(Interaction::Unhovered));
            //         // } else {
            //         return None;
            //         // }
            //     };

            //     let bar = ((bounds.width - position.x) / self.zoom.0 as f32) as usize;

            //     let (index, _datapoint) = self
            //         .datapoints
            //         .clone()
            //         .nth(bar)
            //         .or_else(|| self.datapoints.clone().last())?;

            //     if Some(index) == *bar_hovered {
            //         if matches!(event, Event::Mouse(mouse::Event::CursorMoved { .. })) {
            //             self.cache.clear();
            //             return Some(canvas::Action::request_redraw());
            //         } else {
            //             return None;
            //         }
            //     }

            //     *bar_hovered = Some(index);
            //     self.cache.clear();

            //     Some(canvas::Action::publish(Interaction::Hovered(index)))
            // }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) if cursor.is_over(bounds) => {
                match delta {
                    mouse::ScrollDelta::Lines { y, .. } | mouse::ScrollDelta::Pixels { y, .. } => {
                        // let new_zoom = if y.is_sign_positive() {
                        //     self.zoom.increment()
                        // } else {
                        //     self.zoom.decrement()
                        // };

                        // if new_zoom == self.zoom {
                        //     return None;
                        // }

                        // Some(canvas::Action::publish(Interaction::ZoomChanged(new_zoom)))
                        None
                    }
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let bounds = frame.size();
            let palette = theme.extended_palette();

            let datapoints = self.datapoints.clone().take(22050);

            let datapoints = datapoints.clone().map(|(_i, datapoint)| datapoint);

            let Some(min) = datapoints.clone().min_by(self.cmp) else {
                return;
            };

            let Some(max) = datapoints.clone().max_by(self.cmp) else {
                return;
            };

            let min_value = (self.to_float)(min) as f32;
            let max_value = (self.to_float)(max) as f32;

            let x_range = 0.0..=datapoints.clone().count() as f32;

            let max_value = 20.0;
            let y_range = min_value..=max_value;
            let x_axis = HorizontalAxis::new(x_range, 10);
            let y_axis = VerticalAxis::new(y_range, bounds.height - x_axis.height, 10);

            let pixels_per_unit = y_axis.gauge_length / (y_axis.range.end() - y_axis.range.start());

            let width = bounds.width - f32::from(y_axis.width);
            let max_length = datapoints.clone().count();
            let pixels_per_unit_x = width / max_length as f32;
            let window_size = if pixels_per_unit_x < 1.0 {
                Some((max_length as f32 / width).floor() as usize)
            } else {
                None
            };

            let bar_width = if window_size.is_some() {
                1.0
            } else {
                pixels_per_unit_x
            };

            struct Window<T> {
                value: T,
                pos: usize,
            }

            let mut window = window_size.map(|_| Window {
                value: min_value,
                pos: 0,
            });

            for (i, datapoint) in datapoints.enumerate() {
                let value = if let Some(ref mut window) = window {
                    if window.pos < window_size.unwrap() {
                        // window.value += (self.to_float)(datapoint);
                        window.value = window.value.max((self.to_float)(datapoint));
                        window.pos += 1;
                        continue;
                    } else {
                        // let datapoint = window.value / window.pos as f32;
                        let datapoint = window.value;
                        *window = Window {
                            value: min_value,
                            pos: 0,
                        };
                        datapoint
                    }
                } else {
                    (self.to_float)(datapoint)
                };

                let bar_height = ((value - min_value) * pixels_per_unit) as f32;

                let divider = window_size.unwrap_or(1);
                let bar = Rectangle {
                    x: f32::from(y_axis.width) + bar_width * (i / divider) as f32,
                    y: f32::from(y_axis.height)
                        - x_axis.height
                        - y_axis.min_label_height * 0.5
                        - bar_height,
                    width: bar_width,
                    height: bar_height,
                };

                frame.fill_rectangle(bar.position(), bar.size(), palette.secondary.weak.color);
            }

            frame.with_save(|frame| {
                frame.translate(Vector::new(f32::from(y_axis.width), 0.0));

                x_axis.draw(frame, bounds.width - f32::from(y_axis.width));
            });
            y_axis.draw(frame);
        });

        vec![geometry]
    }
}

struct HorizontalAxis<'a> {
    height: f32,
    tick_amount: usize,
    labels: Vec<Label<'a>>,
}

impl<'a> HorizontalAxis<'a> {
    pub fn new(range: RangeInclusive<f32>, tick_amount: usize) -> Self {
        let length = range.end() - range.start();
        let tick_distance = length / tick_amount as f32;

        let min = *range.start();
        let labels = (0..=tick_amount)
            .into_iter()
            .map(|t| min + t as f32 * tick_distance)
            .map(|l| Label::new(format!("{:.0}", l), 12.0));

        let min_label_height = labels.clone().next().map(|l| l.min_height()).unwrap();

        Self {
            height: min_label_height,
            tick_amount,
            labels: labels.collect(),
        }
    }

    pub fn draw(&self, frame: &mut Frame, target_length: f32) {
        let tick_distance = target_length / self.tick_amount as f32;

        for (i, label) in self.labels.iter().enumerate() {
            let x = i as f32 * tick_distance;
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
}

struct VerticalAxis<'a> {
    range: RangeInclusive<f32>,
    width: Pixels,
    height: Pixels,
    min_label_height: f32,
    gauge_length: f32,
    tick_amount: usize,
    labels: Vec<Label<'a>>,
}

impl<'a> VerticalAxis<'a> {
    pub fn new(range: RangeInclusive<f32>, target_length: f32, tick_amount: usize) -> Self {
        let length = range.end() - range.start();
        let pixels_per_unit = target_length / length;

        let tick_distance = length / tick_amount as f32;

        let min = *range.start();
        let labels = (0..=tick_amount)
            .into_iter()
            .map(|t| min + t as f32 * tick_distance)
            .map(|l| Label::new(format!("{:.0}", l), 12.0));

        let min_label_width = labels
            .clone()
            .map(|l| l.min_width())
            .max_by(f32::total_cmp)
            .unwrap();

        let min_label_height = labels.clone().next().map(|l| l.min_height()).unwrap();

        Self {
            range,
            width: Pixels(min_label_width), // + padding + thickness
            height: Pixels(length * pixels_per_unit + min_label_height), // + padding
            gauge_length: length * pixels_per_unit - min_label_height,
            min_label_height,
            tick_amount,
            labels: labels.collect(),
        }
    }

    pub fn draw(&self, frame: &mut Frame) {
        let tick_distance = self.gauge_length / self.tick_amount as f32;
        for (i, label) in self.labels.iter().enumerate() {
            let y = i as f32 * f32::from(tick_distance);

            frame.fill_text(canvas::Text {
                content: label.content.to_string(),
                position: Point::new(
                    f32::from(self.width),
                    f32::from(self.gauge_length) + self.min_label_height * 0.5 - y,
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
    content: Fragment<'a>,
    bounds: iced::Size,
}

impl<'a> Label<'a> {
    pub fn new(content: impl IntoFragment<'a>, font_size: impl Into<Pixels>) -> Self {
        let content = content.into_fragment();
        let bounds = min_bounds(content.as_ref(), font_size.into());

        Self { content, bounds }
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
        bounds: iced::Size::INFINITY,
        font: Font::MONOSPACE,
        align_x: iced::advanced::text::Alignment::Right,
        align_y: alignment::Vertical::Center,
        shaping: text::Shaping::Advanced,
        wrapping: text::Wrapping::default(),
    };

    let paragraph = Paragraph::with_text(text);
    paragraph.min_bounds()
}
