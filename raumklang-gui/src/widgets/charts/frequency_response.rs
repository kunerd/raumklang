use super::{InteractiveViewport, InteractiveViewportMessage};
use raumklang_core::{dbfs, FrequencyResponse};

use iced::{
    alignment::{Horizontal, Vertical},
    event, mouse,
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        pick_list, Column, Container,
    },
    Element, Length, Size,
};
use plotters::{
    coord::{
        ranged1d::{NoDefaultFormatting, ReversibleRanged, ValueFormatter},
        types::RangedCoordi64,
    },
    prelude::Ranged,
    style::RGBAColor,
};
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartBuilder, ChartWidget, Renderer};

use rustfft::num_complex::{Complex, ComplexFloat};

use std::{fmt::Display, ops::Range};

#[derive(Debug, Clone)]
pub enum Message {
    UnitChanged(FrequencyResponseUnit),
    SmoothingChanged(SmoothingType),
    InteractiveViewport(InteractiveViewportMessage),
}

pub struct FrequencyResponseChart {
    responses: Vec<FrequencyResponseData>,
    unit: FrequencyResponseUnit,
    smoothing: Option<SmoothingType>,
    viewport: InteractiveViewport<FrequencyResponseRange>,
    cache: Cache,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum SmoothingType {
    ThirdOctave,
    SixthOctave,
    TwelfthOctave,
    TwentyFourth,
    FourtyEighth,
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

pub struct FrequencyResponseData {
    graph: Vec<f32>,
    original: FrequencyResponse,
    color: RGBAColor,
}

#[derive(Debug, Clone, Default, PartialEq)]
pub enum FrequencyResponseUnit {
    #[default]
    Frequency,
    Bins,
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

    pub fn view(&self) -> Element<Message> {
        let header = {
            let unit_picker = pick_list(
                &FrequencyResponseUnit::ALL[..],
                Some(self.unit.clone()),
                Message::UnitChanged,
            );

            let smoothing_picker = pick_list(
                &SmoothingType::ALL[..],
                self.smoothing,
                Message::SmoothingChanged,
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

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::UnitChanged(unit) => self.unit = unit,
            Message::SmoothingChanged(smoothing) => {
                self.smoothing = Some(smoothing);
                self.responses.iter_mut().for_each(|r| r.smooth(smoothing));
            }
            Message::InteractiveViewport(msg) => {
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

impl Chart<Message> for FrequencyResponseChart {
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
    ) -> (event::Status, Option<Message>) {
        let (event, msg) = self.viewport.handle_event(event, bounds, cursor);
        (
            event,
            msg.map(Message::InteractiveViewport),
        )
    }
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
