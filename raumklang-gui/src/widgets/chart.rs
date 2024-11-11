use std::{cell::RefCell, ops::Range};

use iced::{
    alignment::{Horizontal, Vertical},
    event, keyboard, mouse,
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        Column, Container,
    },
    Element, Length, Size,
};
use plotters::{
    coord::{
        cartesian::Cartesian2d,
        ranged1d::{NoDefaultFormatting, ReversibleRanged, ValueFormatter},
        types::{RangedCoordf32, RangedCoordi64},
        ReverseCoordTranslate,
    },
    prelude::Ranged,
    style,
};
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartBuilder, ChartWidget, Renderer};

use crate::Signal;

pub struct TimeseriesChart {
    signal: Signal,
    noise_floor: Option<f32>,
    noise_floor_crossing: Option<usize>,
    time_unit: TimeSeriesUnit,
    shift_key_pressed: bool,
    spec: RefCell<Option<Cartesian2d<TimeSeriesRange, RangedCoordf32>>>,
    viewport: Range<i64>,
    cache: Cache,
}

pub struct FrequencyResponseChart {
    data: Vec<f32>,
}

#[derive(Debug, Clone)]
pub enum Message {
    MouseEvent(mouse::Event, iced::Point),
    TimeUnitChanged(TimeSeriesUnit),
    ShiftKeyReleased,
    ShiftKeyPressed,
    NoiseFloorUpdated((f32, usize)),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeSeriesUnit {
    Samples,
    Time,
}

#[derive(Clone)]
pub enum TimeSeriesRange {
    Samples(RangedCoordi64),
    Time(u32, RangedCoordi64),
}

impl TimeSeriesUnit {
    const ALL: [Self; 2] = [TimeSeriesUnit::Samples, TimeSeriesUnit::Time];
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

impl TimeseriesChart {
    pub fn new(signal: Signal, time_unit: TimeSeriesUnit) -> Self {
        let spec = RefCell::new(None);
        let viewport = 0..signal.data.len() as i64;
        Self {
            signal,
            noise_floor: None,
            noise_floor_crossing: None,
            time_unit,
            shift_key_pressed: false,
            viewport,
            spec,
            cache: Cache::new(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let header = widget::row!(widget::pick_list(
            &TimeSeriesUnit::ALL[..],
            Some(self.time_unit.clone()),
            Message::TimeUnitChanged
        ));
        Container::new(
            Column::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .spacing(5)
                .push(header)
                .push(
                    ChartWidget::new(self)
                        .width(Length::Fill)
                        .height(Length::Fill),
                ),
        )
        .width(Length::Fill)
        .height(Length::Fill)
        .align_x(Horizontal::Center)
        .align_y(Vertical::Center)
        .into()
    }

    pub fn update_msg(&mut self, msg: Message) {
        match msg {
            Message::MouseEvent(evt, point) => match evt {
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
            Message::ShiftKeyPressed => {
                self.shift_key_pressed = true;
            }
            Message::ShiftKeyReleased => {
                self.shift_key_pressed = false;
            }
            Message::TimeUnitChanged(u) => {
                self.time_unit = u;
                self.cache.clear();
            }
            Message::NoiseFloorUpdated((nf, nfc)) => {
                self.noise_floor = Some(nf);
                self.noise_floor_crossing = Some(nfc);
                self.cache.clear();
            }
        }
    }

    fn scroll_right(&mut self) {
        let old_viewport = self.viewport.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let mut new_end = old_viewport.end.saturating_add(offset);
        let viewport_max = self.signal.data.len() as i64 + (length / 2);
        if new_end > viewport_max {
            new_end = viewport_max;
        }

        let new_start = new_end - length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn scroll_left(&mut self) {
        let old_viewport = self.viewport.clone();
        let length = old_viewport.end - old_viewport.start;

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = (length as f32 * SCROLL_FACTOR) as i64;

        let mut new_start = old_viewport.start.saturating_sub(offset);
        let viewport_min = -(length / 2);
        if new_start < viewport_min {
            new_start = viewport_min;
        }
        let new_end = new_start + length;

        self.viewport = new_start..new_end;

        self.cache.clear();
    }

    fn zoom_in(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.viewport.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 0.8;
                const LOWER_BOUND: i64 = 256;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len < LOWER_BOUND {
                    new_len = LOWER_BOUND;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.viewport = new_start..new_end;

                self.cache.clear();
            }
        }
    }

    fn zoom_out(&mut self, p: iced::Point) {
        if let Some(spec) = self.spec.borrow().as_ref() {
            let cur_pos = spec.reverse_translate((p.x as i32, p.y as i32));

            if let Some((x, ..)) = cur_pos {
                let old_viewport = self.viewport.clone();
                let old_len = old_viewport.end - old_viewport.start;

                let center_scale = (x - old_viewport.start) as f32 / old_len as f32;

                // FIXME make configurable
                const ZOOM_FACTOR: f32 = 1.2;
                let mut new_len = (old_len as f32 * ZOOM_FACTOR) as i64;
                if new_len >= self.signal.data.len() as i64 {
                    new_len = self.signal.data.len() as i64;
                }

                let new_start = x.saturating_sub((new_len as f32 * center_scale) as i64);
                let new_end = new_start + new_len;
                self.viewport = new_start..new_end;

                self.cache.clear();
            }
        }
    }
}

impl Chart<Message> for TimeseriesChart {
    type State = ();

    #[inline]
    fn draw<R: Renderer, F: Fn(&mut Frame)>(
        &self,
        renderer: &R,
        bounds: Size,
        draw_fn: F,
    ) -> Geometry {
        renderer.draw_cache(&self.cache, bounds, draw_fn)
    }

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use plotters::prelude::*;

        let x_range = match self.time_unit {
            TimeSeriesUnit::Samples => TimeSeriesRange::Samples(self.viewport.clone().into()),
            TimeSeriesUnit::Time => {
                TimeSeriesRange::Time(self.signal.sample_rate, self.viewport.clone().into())
            }
        };

        let min = self
            .signal
            .data
            .iter()
            .fold(f32::INFINITY, |a, b| a.min(*b));

        let max = self
            .signal
            .data
            .iter()
            .fold(f32::NEG_INFINITY, |a, b| a.max(*b));

        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(x_range, min..max)
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                self.signal
                    .data
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, s)| (i as i64, s)),
                &style::RGBColor(2, 125, 66),
            ))
            .unwrap();

        if let Some(nf) = self.noise_floor {
            chart
                .draw_series(LineSeries::new(
                    (0..self.signal.data.len()).map(|i| (i as i64, nf)),
                    &style::RGBColor(0, 0, 128),
                ))
                .unwrap();
        }

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();

        *self.spec.borrow_mut() = Some(chart.as_coord_spec().clone());
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        if let mouse::Cursor::Available(point) = cursor {
            match event {
                canvas::Event::Mouse(evt) if bounds.contains(point) => {
                    let p_origin = bounds.position();
                    let p = point - p_origin;
                    return (
                        event::Status::Captured,
                        Some(Message::MouseEvent(evt, iced::Point::new(p.x, p.y))),
                    );
                }
                canvas::Event::Mouse(_) => {}
                canvas::Event::Touch(_) => {}
                canvas::Event::Keyboard(event) => match event {
                    iced::keyboard::Event::KeyPressed { key, .. } => match key {
                        iced::keyboard::Key::Named(keyboard::key::Named::Shift) => {
                            return (event::Status::Captured, Some(Message::ShiftKeyPressed))
                        }
                        iced::keyboard::Key::Named(_) => {}
                        iced::keyboard::Key::Character(_) => {}
                        iced::keyboard::Key::Unidentified => {}
                    },
                    iced::keyboard::Event::KeyReleased { key, .. } => match key {
                        iced::keyboard::Key::Named(keyboard::key::Named::Shift) => {
                            return (event::Status::Captured, Some(Message::ShiftKeyReleased))
                        }
                        iced::keyboard::Key::Named(_) => {}
                        iced::keyboard::Key::Character(_) => {}
                        iced::keyboard::Key::Unidentified => {}
                    },
                    iced::keyboard::Event::ModifiersChanged(_) => {}
                },
            }
        }
        (event::Status::Ignored, None)
    }
}

impl FrequencyResponseChart {
    pub fn new(data: Vec<f32>) -> Self {
        Self { data }
    }

    pub fn view(&self) -> Element<()> {
        ChartWidget::new(self)
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }
}

impl Chart<()> for FrequencyResponseChart {
    type State = ();

    //#[inline]
    //fn draw<R: Renderer, F: Fn(&mut Frame)>(
    //    &self,
    //    renderer: &R,
    //    bounds: Size,
    //    draw_fn: F,
    //) -> Geometry {
    //    renderer.draw_cache(&self.cache, bounds, draw_fn)
    //}

    fn build_chart<DB: DrawingBackend>(&self, _state: &Self::State, mut builder: ChartBuilder<DB>) {
        use plotters::prelude::*;

        //let x_range = match self.time_unit {
        //    TimeSeriesUnit::Samples => TimeSeriesRange::Samples(self.viewport.clone().into()),
        //    TimeSeriesUnit::Time => {
        //        TimeSeriesRange::Time(self.signal.sample_rate, self.viewport.clone().into())
        //    }
        //};

        let min = self.data.iter().fold(f32::INFINITY, |a, b| a.min(*b));

        let max = self.data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));

        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(0..self.data.len(), min..max)
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                self.data
                    .iter()
                    .cloned()
                    .enumerate(),
                &style::RGBColor(2, 125, 66),
            ))
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();
    }

    //fn update(
    //    &self,
    //    _state: &mut Self::State,
    //    event: canvas::Event,
    //    bounds: iced::Rectangle,
    //    cursor: mouse::Cursor,
    //) -> (event::Status, Option<Message>) {
    //    if let mouse::Cursor::Available(point) = cursor {
    //        match event {
    //            canvas::Event::Mouse(evt) if bounds.contains(point) => {
    //                let p_origin = bounds.position();
    //                let p = point - p_origin;
    //                return (
    //                    event::Status::Captured,
    //                    Some(Message::MouseEvent(evt, iced::Point::new(p.x, p.y))),
    //                );
    //            }
    //            canvas::Event::Mouse(_) => {}
    //            canvas::Event::Touch(_) => {}
    //            canvas::Event::Keyboard(event) => match event {
    //                iced::keyboard::Event::KeyPressed { key, .. } => match key {
    //                    iced::keyboard::Key::Named(keyboard::key::Named::Shift) => {
    //                        return (event::Status::Captured, Some(Message::ShiftKeyPressed))
    //                    }
    //                    iced::keyboard::Key::Named(_) => {}
    //                    iced::keyboard::Key::Character(_) => {}
    //                    iced::keyboard::Key::Unidentified => {}
    //                },
    //                iced::keyboard::Event::KeyReleased { key, .. } => match key {
    //                    iced::keyboard::Key::Named(keyboard::key::Named::Shift) => {
    //                        return (event::Status::Captured, Some(Message::ShiftKeyReleased))
    //                    }
    //                    iced::keyboard::Key::Named(_) => {}
    //                    iced::keyboard::Key::Character(_) => {}
    //                    iced::keyboard::Key::Unidentified => {}
    //                },
    //                iced::keyboard::Event::ModifiersChanged(_) => {}
    //            },
    //        }
    //    }
    //    (event::Status::Ignored, None)
    //}
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
                format!("{} s", *value as f32 / *sample_rate as f32)
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
