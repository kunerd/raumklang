use super::{HorizontalAxis, Offset, VerticalAxis, Zoom};

use iced::{
    Event, Point, Rectangle, Renderer, Size, Vector,
    mouse::{self, ScrollDelta},
    widget::canvas::{self},
};

use std::{cmp::Ordering, ops::RangeInclusive};

#[derive(Debug, Clone)]
pub enum Interaction {
    ZoomChanged(Zoom),
    OffsetChanged(Offset),
}

pub struct Waveform<'a, I, Y, ScaleX>
where
    I: Iterator<Item = Y>,
{
    pub datapoints: I,
    pub cache: &'a canvas::Cache,
    pub cmp: fn(&Y, &Y) -> Ordering,
    pub y_to_float: fn(Y) -> f32,
    pub to_x_scale: ScaleX,
    pub zoom: Zoom,
    pub offset: Offset,
    pub y_range: Option<RangeInclusive<Y>>,
}

#[derive(Default)]
pub struct State {
    shift_pressed: bool,
}

impl<'a, I, Y, ScaleX> canvas::Program<Interaction, iced::Theme> for Waveform<'a, I, Y, ScaleX>
where
    I: Iterator<Item = Y> + Clone + 'a,
    Y: Copy + std::iter::Sum + PartialOrd,
    ScaleX: Fn(f32) -> f32,
{
    type State = State;

    fn update(
        &self,
        state: &mut State,
        event: &Event,
        _bounds: Rectangle,
        _cursor: mouse::Cursor,
    ) -> Option<canvas::Action<Interaction>> {
        match event {
            Event::Keyboard(iced::keyboard::Event::KeyPressed { key, .. }) => {
                if let iced::keyboard::Key::Named(iced::keyboard::key::Named::Shift) = key {
                    state.shift_pressed = true;
                }

                None
            }
            Event::Keyboard(iced::keyboard::Event::KeyReleased { key, .. }) => {
                if let iced::keyboard::Key::Named(iced::keyboard::key::Named::Shift) = key {
                    state.shift_pressed = false;
                }

                None
            }
            Event::Mouse(mouse::Event::WheelScrolled { delta }) => {
                let ScrollDelta::Lines { y, .. } = delta else {
                    return None;
                };

                if state.shift_pressed {
                    let diff = (f32::from(self.zoom) * 44_100_f32).ceil() as isize;

                    let new_offset = if y.is_sign_positive() {
                        self.offset.saturating_add(diff)
                    } else {
                        self.offset.saturating_sub(diff)
                    };

                    if self.offset != new_offset {
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

                    Some(canvas::Action::publish(Interaction::ZoomChanged(new_zoom)))
                }
            }
            _ => None,
        }
    }

    fn draw(
        &self,
        _state: &Self::State,
        renderer: &Renderer,
        theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let palette = theme.extended_palette();

        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let x_min = f32::from(self.offset) * f32::from(self.zoom);
            let x_max = (self.datapoints.clone().count() as f32 + f32::from(self.offset))
                * f32::from(self.zoom);

            let min_index = self.offset.0.clamp(0, isize::MAX) as usize;
            let max_index = min_index
                + self
                    .datapoints
                    .clone()
                    .count()
                    .saturating_add_signed(self.offset.0);

            if max_index < 2 {
                return;
            }

            let x_range = x_min..=x_max;
            let x_axis = HorizontalAxis::new(x_range, &self.to_x_scale, 10);

            let datapoints = self.datapoints.clone().skip(min_index).take(max_index);

            let Some(y_min) = datapoints.clone().min_by(self.cmp) else {
                return;
            };
            let Some(y_max) = datapoints.clone().max_by(self.cmp) else {
                return;
            };

            let y_range = if let Some(range) = self.y_range.as_ref() {
                let y_min = (self.y_to_float)(*range.start());
                let y_max = (self.y_to_float)(*range.end());

                y_min..=y_max
            } else {
                let y_min = (self.y_to_float)(y_min);
                let y_max = (self.y_to_float)(y_max);

                y_min..=y_max
            };
            let y_axis = VerticalAxis::new(y_range, 10);

            let plane = Rectangle::new(
                Point::new(bounds.x, bounds.y),
                Size::new(bounds.width, bounds.height - x_axis.height),
            );

            let pixels_per_unit_x = plane.width / x_axis.length;
            let pixels_per_unit_y = plane.height / y_axis.length;

            let bar_width = if pixels_per_unit_x < 1.0 {
                1.0
            } else {
                pixels_per_unit_x
            };

            for (i, datapoint) in datapoints.enumerate() {
                let value = (self.y_to_float)(datapoint);

                let bar_height = value * pixels_per_unit_y;
                let bar = Rectangle {
                    x: (i as f32 - x_min) * pixels_per_unit_x,
                    y: plane.height + y_axis.min * pixels_per_unit_y,
                    width: bar_width,
                    height: bar_height,
                };

                frame.fill_rectangle(bar.position(), bar.size(), palette.secondary.weak.color);
            }

            frame.with_save(|frame| {
                frame.translate(Vector::new(y_axis.width, 0.0));

                x_axis.draw(frame, plane.width);
            });
        });

        vec![geometry]
    }
}
