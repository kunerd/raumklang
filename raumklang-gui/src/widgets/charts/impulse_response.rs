use super::{
    AmplitudeUnit, InteractiveViewport, InteractiveViewportMessage, TimeSeriesRange, TimeSeriesUnit,
};

use iced::{
    alignment::{Horizontal, Vertical},
    event, mouse,
    widget::{
        self,
        canvas::{self, Cache, Frame, Geometry},
        text, Column, Container,
    },
    Element, Length, Size,
};
use plotters::style::{self};
use plotters_backend::DrawingBackend;
use plotters_iced::{Chart, ChartBuilder, ChartWidget, Renderer};

#[derive(Debug, Clone)]
pub enum Message {
    TimeUnitChanged(TimeSeriesUnit),
    AmplitudeUnitChanged(AmplitudeUnit),
    InteractiveViewport(InteractiveViewportMessage),
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
