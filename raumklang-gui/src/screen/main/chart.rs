use std::{cmp::Ordering, ops::RangeInclusive};

use iced::{
    advanced::{
        graphics::text::Paragraph,
        text::{self, Paragraph as _},
    },
    alignment, mouse,
    widget::{
        canvas::{self, Frame, Path, Stroke},
        container,
        text::{Fragment, IntoFragment},
    },
    window, Element, Event, Font,
    Length::Fill,
    Pixels, Point, Rectangle, Renderer, Theme, Vector,
};

use crate::{
    data::{chart, Samples, Window},
    ui,
};

pub fn impulse_response<'a>(
    window: &'a Window<Samples>,
    impulse_response: &'a ui::ImpulseResponse,
    time_unit: &'a chart::TimeSeriesUnit,
    amplitude_unit: &'a chart::AmplitudeUnit,
    x_range: &'a RangeInclusive<f32>,
    cache: &'a canvas::Cache,
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
            cache,
            cmp: |a, b| a.total_cmp(b),
            x_to_float: |i| i as f32,
            to_x_scale: move |i| match time_unit {
                chart::TimeSeriesUnit::Time => time_scale(i, impulse_response.sample_rate.into()),
                chart::TimeSeriesUnit::Samples => i as f32,
            },
            y_to_float: |s| s,
            to_y_scale: move |s| match amplitude_unit {
                chart::AmplitudeUnit::PercentFullScale => percent_full_scale(s),
                chart::AmplitudeUnit::DezibelFullScale => db_full_scale(s),
            },
            to_string: |s| format!("{s}:0."),
            x_range,
        })
        .width(Fill)
        .height(Fill),
    )
    .into()
}

struct BarChart<'a, I, X, Y, ScaleX, ScaleY>
where
    I: Iterator<Item = (X, Y)>,
    // ToFloat: Fn(Y) -> f32,
{
    window: &'a Window<Samples>,
    datapoints: I,
    cache: &'a canvas::Cache,
    cmp: fn(&Y, &Y) -> Ordering,
    x_to_float: fn(X) -> f32,
    to_x_scale: ScaleX,
    y_to_float: fn(Y) -> f32,
    to_y_scale: ScaleY,
    to_string: fn(Y) -> String,
    x_range: &'a RangeInclusive<f32>,
    // y_range: &'a RangeInclusive<f32>,
    // average: fn(T, u32) -> A,
    // average_to_float: fn(A) -> f64,
    // average_to_string: fn(A) -> String,
    // zoom: Zoom,
}

#[derive(Debug, Clone)]
pub enum Interaction {}

impl<'a, I, X, Y, ScaleX, ScaleY> canvas::Program<Interaction, iced::Theme>
    for BarChart<'a, I, X, Y, ScaleX, ScaleY>
where
    I: Iterator<Item = (X, Y)> + Clone + 'a,
    Y: Copy + std::iter::Sum,
    ScaleX: Fn(f32) -> f32,
    ScaleY: Fn(f32) -> f32,
{
    type State = Option<Point>;

    fn update(
        &self,
        // bar_hovered: &mut Option<timeline::Index>,
        // _state: &mut Self::State,
        last_pos: &mut Option<Point>,
        event: &Event,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Interaction>> {
        match event {
            Event::Mouse(mouse::Event::CursorMoved { .. })
            | Event::Window(window::Event::RedrawRequested(_)) => {
                if let Some(ref mut pos) = last_pos {
                    if let Some(cursor) = cursor.position_in(bounds) {
                        if f32::abs(cursor.x - pos.x) >= 1.0 {
                            *pos = cursor;
                            self.cache.clear();
                            return Some(canvas::Action::request_redraw());
                        }
                    } else {
                        *last_pos = None;
                        self.cache.clear();
                        return Some(canvas::Action::request_redraw());
                    }
                } else {
                    *last_pos = cursor.position()
                }

                None
            }
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
        _last_pos: &Option<Point>,
        renderer: &Renderer,
        theme: &Theme,
        bounds: Rectangle,
        cursor: mouse::Cursor,
    ) -> Vec<canvas::Geometry> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let cursor = cursor.position_in(bounds);

            let bounds = frame.size();
            let palette = theme.extended_palette();

            let x_max = 0.6 * 44_100 as f32;
            let datapoints = self.datapoints.clone().take(x_max.ceil() as usize);
            // let datapoints = self.datapoints.clone();

            // let datapoints = datapoints.clone().map(|(_i, datapoint)| datapoint);
            let datapoints = datapoints.clone().map(|(_i, datapoint)| datapoint);

            let Some(min) = datapoints.clone().min_by(self.cmp) else {
                return;
            };

            let Some(max) = datapoints.clone().max_by(self.cmp) else {
                return;
            };

            let min_value = (self.to_y_scale)((self.y_to_float)(min));
            let max_value = (self.to_y_scale)((self.y_to_float)(max)) + 10.0;

            let x_min = 0.250 * 44_100 as f32;
            let x_max = datapoints.clone().count() as f32;
            // let x_min = (self.to_x_scale)(x_min);
            // let x_max = (self.to_x_scale)(datapoints.clone().count());

            let x_range = -x_min..=x_max;
            let x_axis = HorizontalAxis::new(x_range, &self.to_x_scale, 10);

            let y_range = min_value..=max_value;
            let y_axis = VerticalAxis::new(y_range, 10);

            let width = bounds.width - y_axis.width;
            let pixels_per_unit_x = width / x_axis.length;
            let window_size = if pixels_per_unit_x < 1.0 {
                Some((x_axis.length / width).floor() as usize)
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

            let mut cur_window = window_size.map(|_| Window { value: min, pos: 0 });

            let y_target_length = bounds.height - x_axis.height - y_axis.min_label_height * 0.5;
            let pixels_per_unit = y_target_length / y_axis.length;
            for (i, datapoint) in datapoints.enumerate() {
                let value = if let Some(ref mut cur_window) = cur_window {
                    if cur_window.pos < window_size.unwrap() {
                        // window.value += (self.to_float)(datapoint);
                        cur_window.value = match (self.cmp)(&cur_window.value, &datapoint) {
                            Ordering::Less => datapoint,
                            Ordering::Equal => datapoint,
                            Ordering::Greater => cur_window.value,
                        };
                        cur_window.pos += 1;
                        continue;
                    } else {
                        // let datapoint = window.value / window.pos as f32;
                        let datapoint = cur_window.value;
                        *cur_window = Window { value: min, pos: 0 };
                        datapoint
                    }
                } else {
                    datapoint
                };

                let value = (self.y_to_float)(value);
                let value = (self.to_y_scale)(value);

                let bar_height = (value - min_value) * pixels_per_unit;

                let divider = window_size.unwrap_or(1);
                let bar = Rectangle {
                    x: y_axis.width
                        + (x_min * pixels_per_unit_x)
                        + bar_width * (i / divider) as f32,
                    y: bounds.height - x_axis.height - bar_height,
                    width: bar_width,
                    height: bar_height,
                };

                frame.fill_rectangle(bar.position(), bar.size(), palette.secondary.weak.color);
            }

            let mut window_curve = self.window.curve().map(|(x, y)| (x, (self.to_y_scale)(y)));

            let path = Path::new(|b| {
                if let Some((x, y)) = window_curve.next() {
                    b.move_to(Point {
                        x: y_axis.width + x_min * pixels_per_unit_x + x * pixels_per_unit_x,
                        y: bounds.height - x_axis.height - (y - min_value) * pixels_per_unit,
                    });
                    window_curve.fold(b, |acc, (x, y)| {
                        acc.line_to(Point {
                            x: y_axis.width + x_min * pixels_per_unit_x + x * pixels_per_unit_x,
                            y: bounds.height - x_axis.height - (y - min_value) * pixels_per_unit,
                        });
                        acc
                    });
                }
            });

            frame.stroke(
                &path, //.transform(&Transform2D::new(1.0, 0.0, 0.0, -1.0, 0.0, 0.0)),
                Stroke::default()
                    .with_width(2.0)
                    .with_color(palette.success.weak.color),
            );

            if let Some(cursor) = cursor {
                let path = Path::line(
                    Point {
                        x: cursor.x,
                        y: 0.0,
                    },
                    Point {
                        x: cursor.x,
                        y: bounds.height - x_axis.height,
                    },
                );

                frame.stroke(
                    &path,
                    Stroke::default()
                        .with_width(2.0)
                        .with_color(palette.background.weakest.color),
                );
            }

            frame.with_save(|frame| {
                frame.translate(Vector::new(y_axis.width, 0.0));

                x_axis.draw(frame, bounds.width - y_axis.width);
            });

            y_axis.draw(frame, y_target_length);
        });

        vec![geometry]
    }
}

struct HorizontalAxis<'a> {
    min: f32,
    length: f32,
    height: f32,
    tick_amount: usize,
    labels: Vec<Label<'a>>,
}

impl<'a> HorizontalAxis<'a> {
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
            .map(|t| offset + min + t as f32 * tick_distance)
            .map(|t| {
                let l = (to_scale)(t);
                Label::new(t, format!("{:.0}", l), 12.0)
            });

        let min_label_height = labels.clone().next().map(|l| l.min_height()).unwrap();

        Self {
            min: *range.start(),
            length,
            height: min_label_height,
            tick_amount,
            labels: labels.collect(),
        }
    }

    pub fn draw(&self, frame: &mut Frame, target_length: f32) {
        // let tick_distance = target_length / self.tick_amount as f32;
        let pixels_per_unit = target_length / self.length;

        for label in self.labels.iter() {
            // let x = i as f32 * tick_distance;
            let x = (label.value - self.min) * pixels_per_unit;
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
    min: f32,
    width: f32,
    length: f32,
    min_label_height: f32,
    tick_amount: usize,
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
            tick_amount,
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

fn sample_scale(index: f32, _sample_rate: f32) -> f32 {
    index
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
