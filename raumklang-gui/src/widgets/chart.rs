use std::{
    cell::RefCell,
    fmt::Display,
    ops::{Range, Sub},
    sync::Arc,
};

use iced::{
    alignment::{Horizontal, Vertical},
    event, keyboard, mouse,
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        pick_list, text, Column, Container,
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
    style::{self, RGBAColor},
};
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartBuilder, ChartWidget, Renderer};
use raumklang_core::{dbfs, FrequencyResponse};
use rustfft::{
    num_complex::{Complex, ComplexFloat},
    num_traits::SaturatingSub,
};

#[derive(Debug, Clone)]
pub enum Message {
    TimeUnitChanged(TimeSeriesUnit),
    AmplitudeUnitChanged(AmplitudeUnit),
    InteractiveViewport(InteractiveViewportMessage),
}

#[derive(Debug, Clone)]
pub enum SignalChartMessage {
    TimeUnitChanged(TimeSeriesUnit),
    InteractiveViewport(InteractiveViewportMessage),
}

pub struct SignalChart {
    signal: Arc<raumklang_core::Measurement>,
    time_unit: TimeSeriesUnit,
    viewport: InteractiveViewport<TimeSeriesRange>,
    cache: Cache,
}

pub struct ImpulseResponseChart {
    impulse_response: raumklang_core::ImpulseResponse,
    window: Option<Vec<f32>>,
    noise_floor: Option<f32>,
    amplitude_unit: AmplitudeUnit,
    time_unit: TimeSeriesUnit,
    viewport: InteractiveViewport<TimeSeriesRange>,
    cache: Cache,
}

pub struct FrequencyResponseChart {
    responses: Vec<FrequencyResponseData>,
    unit: FrequencyResponseUnit,
    smoothing: Option<SmoothingType>,
    viewport: InteractiveViewport<FrequencyResponseRange>,
    cache: Cache,
}

pub struct FrequencyResponseData {
    graph: Vec<f32>,
    original: FrequencyResponse,
    color: RGBAColor,
}

impl FrequencyResponseData {
    pub fn new(original: FrequencyResponse, color: RGBAColor) -> Self {
        let graph: Vec<_> = original
            .data
            .iter()
            .cloned()
            .map(Complex::re)
            .map(f32::abs)
            .map(dbfs)
            .collect();

        Self {
            graph,
            original,
            color,
        }
    }

    fn smooth(&mut self, smoothing: SmoothingType) {
        let data: Vec<f32> = self
            .original
            .data
            .iter()
            .cloned()
            .map(Complex::re)
            .map(f32::abs)
            .map(dbfs)
            .collect();

        let mut new_data = vec![];
        for i in 0..data.len() {
            let center_bin = |i: usize| -> f32 {
                2.0_f32
                    .powf(i as f32 / usize::from(smoothing) as f32)
                    .floor()
            };

            let lower_bin =
                f32::sqrt(center_bin(i.saturating_sub(1)) * center_bin(i)).floor() as usize;
            let mut upper_bin = f32::sqrt(center_bin(i) * center_bin(usize::min(data.len(), i + 1)))
                .floor() as usize;

            if lower_bin >= data.len() {
                break;
            }

            if upper_bin >= data.len() {
                upper_bin = data.len();
            }

            // TODO: include min values
            //let min = data[lower_bin..upper_bin]
            //    .iter()
            //    .cloned()
            //    .reduce(f32::min)
            //    .unwrap_or(f32::NEG_INFINITY);

            let max = data[lower_bin..upper_bin].iter().cloned().reduce(f32::max);

            if let Some(max) = max {
                new_data.push(((lower_bin..upper_bin).len(), max));
            }
        }

        self.graph = vec![];
        let mut prev = f32::NEG_INFINITY;
        let mut iter = new_data.iter().peekable();
        while let Some((n, cur)) = iter.next() {
            if let Some((_, next)) = iter.peek() {
                for i in 0..*n {
                    let weight = i as f32 / *n as f32;
                    let intermediate = interpolation::quad_bez(&prev, cur, next, &(weight / 2.0));
                    self.graph
                        .push(interpolation::quad_bez(&prev, &intermediate, cur, &weight));
                }
                prev = *cur;
            }
        }
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SmoothingType {
    ThirdOctave,
    SixthOctave,
    TwelfthOctave,
    TwentyFourth,
    FourtyEighth,
}

#[derive(Debug, Clone)]
pub enum FrequencyResponseChartMessage {
    UnitChanged(FrequencyResponseUnit),
    SmoothingChanged(SmoothingType),
    InteractiveViewport(InteractiveViewportMessage),
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

#[derive(Debug, Clone, Default, PartialEq)]
pub enum FrequencyResponseUnit {
    #[default]
    Frequency,
    Bins,
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
    const ALL: [AmplitudeUnit; 2] = [
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

impl SignalChart {
    pub fn new(signal: &raumklang_core::Measurement, time_unit: TimeSeriesUnit) -> Self {
        let length = signal.duration() as i64;
        let viewport = InteractiveViewport::new(0..length);
        Self {
            signal: Arc::new(signal.clone()),
            time_unit,
            viewport,
            cache: Cache::new(),
        }
    }

    pub fn view(&self) -> Element<SignalChartMessage> {
        let header = widget::row!(widget::pick_list(
            &TimeSeriesUnit::ALL[..],
            Some(self.time_unit.clone()),
            SignalChartMessage::TimeUnitChanged
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

    pub fn update_msg(&mut self, msg: SignalChartMessage) {
        match msg {
            SignalChartMessage::TimeUnitChanged(u) => {
                self.time_unit = u;
                self.cache.clear();
            }
            SignalChartMessage::InteractiveViewport(msg) => {
                self.viewport.update(msg);
                self.cache.clear()
            }
        }
    }
}

impl Chart<SignalChartMessage> for SignalChart {
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

        let range = self.viewport.range().clone().into();
        let x_range = match self.time_unit {
            TimeSeriesUnit::Samples => TimeSeriesRange::Samples(range),
            TimeSeriesUnit::Time => TimeSeriesRange::Time(self.signal.sample_rate(), range),
        };

        let min = self.signal.iter().cloned().reduce(f32::min).unwrap();

        let max = self.signal.iter().cloned().reduce(f32::max).unwrap();

        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(x_range, min..max)
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                self.signal
                    .iter()
                    .cloned()
                    .enumerate()
                    .map(|(i, s)| (i as i64, s)),
                &style::RGBColor(2, 125, 66),
            ))
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();

        self.viewport.set_spec(chart.as_coord_spec().clone());
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<SignalChartMessage>) {
        let (event, msg) = self.viewport.handle_event(event, bounds, cursor);
        (event, msg.map(SignalChartMessage::InteractiveViewport))
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

#[derive(Clone)]
enum FrequencyResponseRange {
    Frequency {
        sample_rate: u32,
        fft_size: usize,
        range: RangedCoordi64,
    },
    Bins(RangedCoordi64),
}

impl FrequencyResponseUnit {
    const ALL: [Self; 2] = [
        FrequencyResponseUnit::Bins,
        FrequencyResponseUnit::Frequency,
    ];
}

impl Display for FrequencyResponseUnit {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                FrequencyResponseUnit::Frequency => "Hz",
                FrequencyResponseUnit::Bins => "FFT Bins",
            }
        )
    }
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

impl SmoothingType {
    pub const ALL: [SmoothingType; 5] = [
        SmoothingType::ThirdOctave,
        SmoothingType::SixthOctave,
        SmoothingType::TwelfthOctave,
        SmoothingType::TwentyFourth,
        SmoothingType::FourtyEighth,
    ];
}

impl From<SmoothingType> for usize {
    fn from(value: SmoothingType) -> Self {
        match value {
            SmoothingType::ThirdOctave => 3,
            SmoothingType::SixthOctave => 6,
            SmoothingType::TwelfthOctave => 12,
            SmoothingType::TwentyFourth => 24,
            SmoothingType::FourtyEighth => 48,
        }
    }
}

impl Display for SmoothingType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "{}",
            match self {
                SmoothingType::ThirdOctave => "1/3 Octave",
                SmoothingType::SixthOctave => "1/6 Octave",
                SmoothingType::TwelfthOctave => "1/12 Octave",
                SmoothingType::TwentyFourth => "1/24 Octave",
                SmoothingType::FourtyEighth => "1/48 Octave",
            }
        )
    }
}

impl ImpulseResponseChart {
    pub fn new(
        impulse_response: raumklang_core::ImpulseResponse,
        time_unit: TimeSeriesUnit,
    ) -> Self {
        let length = impulse_response.data.len() as i64;
        let viewport = InteractiveViewport::new(0..length);
        Self {
            impulse_response,
            window: None,
            noise_floor: None,
            amplitude_unit: AmplitudeUnit::DezibelFullScale,
            time_unit,
            viewport,
            cache: Cache::new(),
        }
    }

    pub fn view(&self) -> Element<Message> {
        let header = widget::row!(
            text("Amplitude unit:"),
            widget::pick_list(
                &AmplitudeUnit::ALL[..],
                Some(&self.amplitude_unit),
                Message::AmplitudeUnitChanged
            ),
            text("Time unit:"),
            widget::pick_list(
                &TimeSeriesUnit::ALL[..],
                Some(&self.time_unit),
                Message::TimeUnitChanged
            )
        );
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
            Message::TimeUnitChanged(u) => {
                self.time_unit = u;
                self.cache.clear();
            }
            Message::AmplitudeUnitChanged(u) => {
                self.amplitude_unit = u;
                self.cache.clear();
            }
            Message::InteractiveViewport(msg) => {
                self.viewport.update(msg);
                self.cache.clear()
            }
        }
    }
}

impl Chart<Message> for ImpulseResponseChart {
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

        let range = self.viewport.range().clone().into();
        let x_range = match self.time_unit {
            TimeSeriesUnit::Samples => TimeSeriesRange::Samples(range),
            TimeSeriesUnit::Time => TimeSeriesRange::Time(self.impulse_response.sample_rate, range),
        };

        let data: Vec<_> = self
            .impulse_response
            .data
            .iter()
            .map(|s| s.re.abs())
            .collect();

        let max = data
            .iter()
            .map(|s| s.powi(2).sqrt())
            .fold(f32::NEG_INFINITY, |a, b| a.max(b));

        // FIXME: precompute on amplitude change
        let processed_data: Vec<_> = match &self.amplitude_unit {
            AmplitudeUnit::PercentFullScale => data.iter().map(|s| *s / max * 100f32).collect(),
            AmplitudeUnit::DezibelFullScale => data
                .iter()
                .map(|s| 20f32 * f32::log10(s.abs() / max))
                .collect(),
        };

        let min = processed_data.iter().cloned().reduce(f32::min).unwrap();

        let max = processed_data.iter().cloned().reduce(f32::max).unwrap();

        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(30)
            .build_cartesian_2d(x_range, min..max)
            .unwrap();

        chart
            .draw_series(LineSeries::new(
                processed_data
                    .into_iter()
                    .enumerate()
                    .map(|(i, s)| (i as i64, s)),
                &style::RGBColor(2, 125, 66),
            ))
            .unwrap();

        if let Some(nf) = self.noise_floor {
            chart
                .draw_series(LineSeries::new(
                    (0..data.len()).map(|i| (i as i64, nf)),
                    &style::RGBColor(0, 0, 128),
                ))
                .unwrap();
        }

        if let Some(window) = &self.window {
            chart
                .draw_series(LineSeries::new(
                    window
                        .iter()
                        .cloned()
                        .enumerate()
                        .map(|(i, s)| (i as i64, s)),
                    &style::RGBColor(200, 0, 0),
                ))
                .unwrap();
        }

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();

        self.viewport.set_spec(chart.as_coord_spec().clone());
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<Message>) {
        let (event, msg) = self.viewport.handle_event(event, bounds, cursor);
        (event, msg.map(Message::InteractiveViewport))
    }
}

impl FrequencyResponseChart {
    pub fn new(frequency_response: FrequencyResponseData) -> Self {
        let viewport = InteractiveViewport::new(0..frequency_response.graph.len() as i64);

        let responses = vec![frequency_response];

        Self {
            responses,
            unit: FrequencyResponseUnit::default(),
            smoothing: None,
            viewport,
            cache: Cache::new(),
        }
    }

    pub fn view(&self) -> Element<FrequencyResponseChartMessage> {
        let header = {
            let unit_picker = pick_list(
                &FrequencyResponseUnit::ALL[..],
                Some(self.unit.clone()),
                FrequencyResponseChartMessage::UnitChanged,
            );

            let smoothing_picker = pick_list(
                &SmoothingType::ALL[..],
                self.smoothing,
                FrequencyResponseChartMessage::SmoothingChanged,
            );

            widget::row!(unit_picker, smoothing_picker)
        };

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

    pub fn update(&mut self, msg: FrequencyResponseChartMessage) {
        match msg {
            FrequencyResponseChartMessage::UnitChanged(unit) => self.unit = unit,
            FrequencyResponseChartMessage::SmoothingChanged(smoothing) => {
                self.smoothing = Some(smoothing);
                self.responses.iter_mut().for_each(|r| r.smooth(smoothing));
            }
            FrequencyResponseChartMessage::InteractiveViewport(msg) => {
                self.viewport.update(msg);
                self.cache.clear();
            }
        }
    }

    pub fn update_data(&mut self, iter: impl Iterator<Item = FrequencyResponseData>) {
        self.responses = iter.collect();

        if let Some(smoothing) = self.smoothing {
            self.responses.iter_mut().for_each(|r| r.smooth(smoothing))
        }

        self.cache.clear();
    }

    pub fn from_iter(
        mut iter: impl Iterator<Item = FrequencyResponseData>,
    ) -> Option<FrequencyResponseChart> {
        if let Some(response) = iter.next() {
            let mut chart = FrequencyResponseChart::new(response);
            chart.responses.extend(iter);
            Some(chart)
        } else {
            None
        }
    }
}

impl Chart<FrequencyResponseChartMessage> for FrequencyResponseChart {
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
        let mut iter = self.responses.iter();

        let Some(response) = iter.next() else {
            return;
        };

        let range = self.viewport.range().clone().into();
        let x_range = match self.unit {
            FrequencyResponseUnit::Frequency => FrequencyResponseRange::Frequency {
                sample_rate: response.original.sample_rate,
                fft_size: response.graph.len(),
                range,
            },
            FrequencyResponseUnit::Bins => FrequencyResponseRange::Bins(range),
        };

        //let min = self.data.iter().fold(f32::INFINITY, |a, b| a.min(*b));
        //let max = self.data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));
        let min = -70f32;
        let max = 3f32;
        let y_range = min..max;

        let mut chart = builder
            .margin(5)
            .x_label_area_size(30)
            .y_label_area_size(50)
            .build_cartesian_2d(x_range, y_range)
            .unwrap();

        let mut response = response;
        loop {
            chart
                .draw_series(LineSeries::new(
                    response
                        .graph
                        .iter()
                        .enumerate()
                        .map(|(i, s)| (i as i64, *s)),
                    response.color,
                ))
                .unwrap();

            response = if let Some(response) = iter.next() {
                response
            } else {
                break;
            }
        }

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();

        self.viewport.set_spec(chart.as_coord_spec().clone())
    }

    fn update(
        &self,
        _state: &mut Self::State,
        event: canvas::Event,
        bounds: iced::Rectangle,
        cursor: mouse::Cursor,
    ) -> (event::Status, Option<FrequencyResponseChartMessage>) {
        let (event, msg) = self.viewport.handle_event(event, bounds, cursor);
        (
            event,
            msg.map(FrequencyResponseChartMessage::InteractiveViewport),
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

impl FrequencyResponseRange {
    fn range(&self) -> &RangedCoordi64 {
        match self {
            FrequencyResponseRange::Bins(range) => range,
            FrequencyResponseRange::Frequency { range, .. } => range,
        }
    }
}

impl ValueFormatter<i64> for FrequencyResponseRange {
    fn format_ext(&self, value: &i64) -> String {
        match self {
            FrequencyResponseRange::Bins(_) => format!("{}", value),
            FrequencyResponseRange::Frequency {
                sample_rate,
                fft_size,
                ..
            } => {
                let frequency = *value as f32 * ((*sample_rate as f32 / 2.0) / *fft_size as f32);
                format!("{frequency}")
            }
        }
    }
}

impl Ranged for FrequencyResponseRange {
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

impl ReversibleRanged for FrequencyResponseRange {
    fn unmap(&self, input: i32, limit: (i32, i32)) -> Option<Self::ValueType> {
        let range = match self {
            FrequencyResponseRange::Frequency { range, .. } => range,
            FrequencyResponseRange::Bins(range) => range,
        };

        range.unmap(input, limit)
    }
}
