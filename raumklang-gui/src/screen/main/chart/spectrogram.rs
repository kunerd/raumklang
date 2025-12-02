use crate::{data, screen::main::chart::Scale};

use super::{HorizontalAxis, Offset, VerticalAxis, Zoom};

use iced::{
    mouse::{self, ScrollDelta},
    widget::canvas::{self},
    Event, Point, Rectangle, Renderer, Size, Vector,
};
use raumklang_core::dbfs;

#[derive(Debug, Clone)]
pub enum Interaction {
    ZoomChanged(Zoom),
    OffsetChanged(Offset),
}

pub struct Spectrogram<'a> {
    pub datapoints: &'a data::Spectrogram,
    pub cache: &'a canvas::Cache,
    // pub cmp: fn(&Y, &Y) -> Ordering,
    // pub y_to_float: fn(Y) -> f32,
    // pub to_x_scale: ScaleX,
    pub zoom: Zoom,
    pub offset: Offset,
}

#[derive(Default)]
pub struct State {
    shift_pressed: bool,
}

impl<'a> canvas::Program<Interaction, iced::Theme> for Spectrogram<'a> {
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
                    let diff = (f32::from(self.zoom) * 1000.0).ceil() as isize;

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
        _theme: &iced::Theme,
        bounds: Rectangle,
        _cursor: iced::advanced::mouse::Cursor,
    ) -> Vec<canvas::Geometry<Renderer>> {
        let geometry = self.cache.draw(renderer, bounds.size(), |frame| {
            let frequency_responses = self.datapoints.iter();

            let Some(first) = frequency_responses.clone().next() else {
                return;
            };

            let max_bin = first.data.iter().count();
            let first = first.data.iter().take(max_bin);

            let x_min = f32::from(self.offset) * f32::from(self.zoom);
            let x_max = (max_bin as f32 + f32::from(self.offset)) * f32::from(self.zoom);

            let min_index = x_min.floor() as usize;
            let max_index = x_max.floor() as usize;

            dbg!(min_index);

            let x_range = x_min..=x_max;

            let labels = [0, 20, 50, 100, 1000, 10_000, 20_000]
                .into_iter()
                .map(|l| l as f32);

            let x_axis = HorizontalAxis::with_labels(x_range, &|s| s, labels).scale(Scale::Log);

            let y_min = 0.0;
            let y_max = self.datapoints.iter().count() as f32;

            let y_range = y_min..=y_max;
            let y_axis = VerticalAxis::new(y_range, 10);

            let plane = Rectangle::new(
                Point::new(bounds.x, bounds.y),
                Size::new(bounds.width, bounds.height - x_axis.height),
            );

            let pixels_per_unit_x = plane.width / x_axis.length;
            let pixels_per_unit_y = plane.height / y_axis.length;

            let gradient = colorous::TURBO;

            for (si, fr) in frequency_responses.enumerate() {
                for (i, s) in fr
                    .data
                    .iter()
                    .skip(min_index)
                    .take(max_index)
                    .copied()
                    .map(dbfs)
                    .map(|s| 1.0 - s.clamp(-50.0, 0.0) / -40.0)
                    .enumerate()
                {
                    let color = gradient.eval_continuous(s.into());

                    let y = plane.height
                        - x_axis.height
                        - si as f32 * pixels_per_unit_y
                        - pixels_per_unit_y;

                    let log_scale = |p: f32| (p.log10() / x_axis.length.log10()) * x_axis.length;

                    let width = log_scale((i + 1) as f32)
                        - log_scale(i as f32).clamp(0.0, f32::MAX) * pixels_per_unit_x;

                    let pixel = Rectangle {
                        x: log_scale(i as f32) * pixels_per_unit_x,
                        y,
                        width,
                        height: pixels_per_unit_y,
                    };

                    frame.fill_rectangle(
                        pixel.position(),
                        pixel.size(),
                        iced::Color::from_rgb8(color.r, color.g, color.b),
                    );
                }
            }

            frame.with_save(|frame| {
                frame.translate(Vector::new(y_axis.width, 0.0));

                x_axis.draw(frame, plane.width);
            });

            // y_axis.draw(frame, plane.height);
        });

        vec![geometry]
    }
}
