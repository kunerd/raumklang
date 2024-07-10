use std::ops::Range;

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{
        self,
        canvas::{Cache, Frame, Geometry},
        Column, Container,
    },
    Element, Length, Size,
};
use plotters::{
    coord::{
        ranged1d::{NoDefaultFormatting, ValueFormatter},
        types::RangedCoordusize,
    },
    prelude::Ranged,
};
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartBuilder, ChartWidget, Renderer};

use crate::Signal;

pub enum TimeSeriesRange {
    Samples(RangedCoordusize),
    Time(u32, RangedCoordusize),
}

impl ValueFormatter<usize> for TimeSeriesRange {
    fn format_ext(&self, value: &usize) -> String {
        match self {
            TimeSeriesRange::Samples(_) => format!("{}", value),
            TimeSeriesRange::Time(sample_rate, _) => {
                format!("{}", *value as f32 / *sample_rate as f32)
            }
        }
    }
}

impl Ranged for TimeSeriesRange {
    type FormatOption = NoDefaultFormatting;

    type ValueType = usize;

    fn map(&self, value: &Self::ValueType, limit: (i32, i32)) -> i32 {
        match self {
            TimeSeriesRange::Samples(ranged) => ranged.map(value, limit),
            TimeSeriesRange::Time(_sample_rate, ranged) => ranged.map(value, limit),
        }
    }

    fn key_points<Hint: plotters::coord::ranged1d::KeyPointHint>(
        &self,
        hint: Hint,
    ) -> Vec<Self::ValueType> {
        match self {
            TimeSeriesRange::Samples(ranged) => ranged.key_points(hint),
            TimeSeriesRange::Time(_, ranged) => ranged.key_points(hint),
        }
    }

    fn range(&self) -> Range<Self::ValueType> {
        match self {
            TimeSeriesRange::Samples(ranged) => ranged.range(),
            TimeSeriesRange::Time(_, ranged) => ranged.range(),
        }
    }

    fn axis_pixel_range(&self, limit: (i32, i32)) -> Range<i32> {
        if limit.0 < limit.1 {
            limit.0..limit.1
        } else {
            limit.1..limit.0
        }
    }
}

pub struct TimeseriesChartNew {
    cache: Cache,
    signal: Signal,
    time_unit: TimeSeriesUnit,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeSeriesUnit {
    Samples,
    Time,
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

#[derive(Debug, Clone)]
pub enum TimeSeriesMessageNew {
    TimeUnitChanged(TimeSeriesUnit),
}

impl TimeseriesChartNew {
    pub fn new(signal: Signal, time_unit: TimeSeriesUnit) -> Self {
        Self {
            signal,
            time_unit,
            cache: Cache::new(),
        }
    }

    pub fn view(&self) -> Element<TimeSeriesMessageNew> {
        let header = widget::row!(widget::pick_list(
            &TimeSeriesUnit::ALL[..],
            Some(self.time_unit.clone()),
            TimeSeriesMessageNew::TimeUnitChanged
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

    pub fn update_msg(&mut self, msg: TimeSeriesMessageNew) {
        match msg {
            TimeSeriesMessageNew::TimeUnitChanged(u) => {
                self.time_unit = u;
                self.cache.clear();
            }
        }
    }
}

impl Chart<TimeSeriesMessageNew> for TimeseriesChartNew {
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
            TimeSeriesUnit::Samples => TimeSeriesRange::Samples((0..self.signal.data.len()).into()),
            TimeSeriesUnit::Time => {
                TimeSeriesRange::Time(self.signal.sample_rate, (0..self.signal.data.len()).into())
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
                self.signal.data.iter().cloned().enumerate(),
                &RED,
            ))
            .unwrap();

        chart
            .configure_mesh()
            .disable_mesh()
            //.disable_axes()
            .draw()
            .unwrap();
    }
}
