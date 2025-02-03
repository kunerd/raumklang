//pub mod frequency_response;
//pub mod impulse_response;
//pub mod measurement;

use iced::{event, keyboard, mouse, widget::canvas};
use plotters::{
    coord::{
        cartesian::Cartesian2d,
        ranged1d::{NoDefaultFormatting, ReversibleRanged, ValueFormatter},
        types::{RangedCoordf32, RangedCoordi64},
        ReverseCoordTranslate,
    },
    prelude::Ranged,
};
use rustfft::num_traits::SaturatingSub;

use std::{
    cell::RefCell,
    ops::{Range, Sub},
};

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum TimeSeriesUnit {
    #[default]
    Time,
    Samples,
}

#[derive(Clone)]
pub enum TimeSeriesRange {
    Samples(RangedCoordi64),
    Time(u32, RangedCoordi64),
}

#[derive(Debug, Clone)]
pub enum InteractiveViewportMessage {
    MouseEvent(mouse::Event, iced::Point),
    ShiftKeyReleased,
    ShiftKeyPressed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AmplitudeUnit {
    #[default]
    PercentFullScale,
    DezibelFullScale,
}

impl AmplitudeUnit {
    pub const ALL: [AmplitudeUnit; 2] = [
        AmplitudeUnit::PercentFullScale,
        AmplitudeUnit::DezibelFullScale,
    ];
}

impl std::fmt::Display for AmplitudeUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                AmplitudeUnit::PercentFullScale => "%FS",
                AmplitudeUnit::DezibelFullScale => "dbFS",
            }
        )
    }
}

struct InteractiveViewport<R>
where
    R: Ranged + ReversibleRanged,
{
    max_len: i64,
    range: Range<i64>,
    shift_key_pressed: bool,
    spec: RefCell<Option<Cartesian2d<R, RangedCoordf32>>>,
}

impl<R> InteractiveViewport<R>
where
    R: Ranged<ValueType = i64> + ReversibleRanged,
    <R as Ranged>::ValueType: Sub<i64> + SaturatingSub,
{
    fn new(range: Range<i64>) -> Self {
        Self {
            max_len: range.end,
            range,
            shift_key_pressed: false,
            spec: RefCell::new(None),
        }
    }

    fn update(&mut self, msg: InteractiveViewportMessage) {
        match msg {
            InteractiveViewportMessage::MouseEvent(evt, point) => match evt {
                mouse::Event::CursorEntered => {}
                mouse::Event::CursorLeft => {}
                mouse::Event::CursorMoved { position: _ } => {}
                mouse::Event::ButtonPressed(_) => {}
                mouse::Event::ButtonReleased(_) => {}
                mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Pixels { x: _, y: _ },
                } => {}
                mouse::Event::WheelScrolled {
                    delta: mouse::ScrollDelta::Lines { y, .. },
                } => {
                    match self.shift_key_pressed {
                        true => {
                            // y is always zero in iced 0.10
                            if y.is_sign_positive() {
                                self.scroll_right();
                            } else {
                                self.scroll_left();
                            }
                        }
                        false => {
                            // y is always zero in iced 0.10
                            if y.is_sign_positive() {
                                self.zoom_in(point);
                            } else {
                                self.zoom_out(point);
                            }
                        }
                    }
                }
            },
            InteractiveViewportMessage::ShiftKeyPressed => {
                self.shift_key_pressed = true;
            }
            InteractiveViewportMessage::ShiftKeyReleased => {
                self.shift_key_pressed = false;
            }
        }
    }

    fn scroll_right(&mut self) {
        let old_viewport = self.range.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let mut new_end = old_viewport.end.saturating_add(offset);
        let viewport_max = self.max_len + (length / 2);
        if new_end > viewport_max {
            new_end = viewport_max;
        }

        let new_start = new_end - length;

        self.range = new_start..new_end;
    }

    fn scroll_left(&mut self) {
        let old_viewport = self.range.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let mut new_start = old_viewport.start.saturating_sub(offset);
        let viewport_min = -(length / 2);
        if new_start < viewport_min {
            new_start = viewport_min;
        }
        let new_end = new_start + length;

        self.range = new_start..new_end;
    }

    fn zoom_in(&mut self, mouse_pos: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((mouse_pos.x as i32, mouse_pos.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.range.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale: f32 = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 0.8;
                const LOWER_BOUND: i64 = 50;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len < LOWER_BOUND {
                    new_len = LOWER_BOUND;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.range = new_start..new_end;
            }
        }
    }

    fn zoom_out(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.range.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 1.2;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len >= self.max_len {
                    new_len = self.max_len;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.range = new_start..new_end;
            }
        }
    }

    fn range(&self) -> &Range<i64> {
        &self.range
    }

    fn set_spec(&self, spec: Cartesian2d<R, RangedCoordf32>) {
        *self.spec.borrow_mut() = Some(spec);
    }

    fn handle_event(
        &self,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<InteractiveViewportMessage>) {
        let maybe_msg = if let mouse::Cursor::Available(point) = cursor {
            match event {
                canvas::Event::Mouse(evt) if bounds.contains(point) => {
                    let p_origin = bounds.position();
                    let p = point - p_origin;
                    Some(InteractiveViewportMessage::MouseEvent(
                        evt,
                        iced::Point::new(p.x, p.y),
                    ))
                }
                canvas::Event::Mouse(_) => None,
                canvas::Event::Touch(_) => None,
                canvas::Event::Keyboard(event) => match event {
                    iced::keyboard::Event::KeyPressed { key, .. } => match key {
                        iced::keyboard::Key::Named(keyboard::key::Named::Shift) => {
                            Some(InteractiveViewportMessage::ShiftKeyPressed)
                        }
                        iced::keyboard::Key::Named(_) => None,
                        iced::keyboard::Key::Character(_) => None,
                        iced::keyboard::Key::Unidentified => None,
                    },
                    iced::keyboard::Event::KeyReleased { key, .. } => match key {
                        iced::keyboard::Key::Named(keyboard::key::Named::Shift) => {
                            Some(InteractiveViewportMessage::ShiftKeyReleased)
                        }
                        iced::keyboard::Key::Named(_) => None,
                        iced::keyboard::Key::Character(_) => None,
                        iced::keyboard::Key::Unidentified => None,
                    },
                    iced::keyboard::Event::ModifiersChanged(_) => None,
                },
            }
        } else {
            None
        };

        match maybe_msg {
            Some(msg) => (event::Status::Captured, Some(msg)),
            None => (event::Status::Ignored, None),
        }
    }
}

impl TimeSeriesUnit {
    pub const ALL: [Self; 2] = [TimeSeriesUnit::Samples, TimeSeriesUnit::Time];
}

impl std::fmt::Display for TimeSeriesUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                TimeSeriesUnit::Samples => "Samples",
                TimeSeriesUnit::Time => "Time",
            }
        )
    }
}

impl TimeSeriesRange {
    fn range(&self) -> &RangedCoordi64 {
        match self {
            TimeSeriesRange::Samples(range) => range,
            TimeSeriesRange::Time(_, range) => range,
        }
    }
}

impl ValueFormatter<i64> for TimeSeriesRange {
    fn format_ext(&self, value: &i64) -> String {
        match self {
            TimeSeriesRange::Samples(_) => format!("{}", value),
            TimeSeriesRange::Time(sample_rate, _) => {
                format!(
                    "{}",
                    (*value as f32 / *sample_rate as f32 * 1000f32).round()
                )
            }
        }
    }
}

impl Ranged for TimeSeriesRange {
    type FormatOption = NoDefaultFormatting;

    type ValueType = i64;

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        self.range().map(value, limit)
    }

    fn key_points<Hint: plotters::coord::ranged1d::KeyPointHint>(
        &self,
        hint: Hint,
    ) -> Vec<Self::ValueType> {
        self.range().key_points(hint)
    }

    fn range(&self) -> Range<Self::ValueType> {
        self.range().range()
    }

    fn axis_pixel_range(&self, limit: (i32, i32)) -> Range<i32> {
        if limit.0 < limit.1 {
            limit.0..limit.1
        } else {
            limit.1..limit.0
        }
    }
}

impl ReversibleRanged for TimeSeriesRange {
    fn unmap(&self, input: i32, limit: (i32, i32)) -> Option<Self::ValueType> {
        let range = match self {
            TimeSeriesRange::Samples(range) => range,
            TimeSeriesRange::Time(_, range) => range,
        };

        range.unmap(input, limit)
    }
}
