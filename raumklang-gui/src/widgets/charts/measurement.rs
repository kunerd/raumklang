use std::sync::Arc;

use iced::{
    alignment::{Horizontal, Vertical},
    event, mouse,
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        Column, Container,
    },
    Element, Length, Size,
};
use plotters::style;
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartBuilder, ChartWidget, Renderer};

use super::{InteractiveViewport, InteractiveViewportMessage, TimeSeriesRange, TimeSeriesUnit};

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
