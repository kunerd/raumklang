use super::chart;

use crate::{
    data::{self},
    ui::ImpulseResponse,
};

use iced::{
    widget::{canvas, column, container, pick_list, row, stack, text},
    Alignment, Color, Element, Length,
};

use std::ops::RangeInclusive;

#[derive(Debug, Clone)]
pub enum Message {
    Chart(ChartOperation),
}

#[derive(Debug, Clone)]
pub enum ChartOperation {
    TimeUnitChanged(data::chart::TimeSeriesUnit),
    AmplitudeUnitChanged(data::chart::AmplitudeUnit),
    Interaction(chart::Interaction),
}

#[derive(Debug, Default)]
pub struct Chart {
    pub x_range: Option<RangeInclusive<f32>>,
    shift_key_pressed: bool,
    pub amplitude_unit: data::chart::AmplitudeUnit,
    pub time_unit: data::chart::TimeSeriesUnit,
    pub cache: canvas::Cache,
    pub line_cache: canvas::Cache,
    pub zoom: chart::Zoom,
    pub offset: i64,
}

impl Chart {
    pub(crate) fn update(&mut self, chart_operation: ChartOperation) {
        match chart_operation {
            ChartOperation::TimeUnitChanged(time_unit) => self.time_unit = time_unit,
            ChartOperation::AmplitudeUnitChanged(amplitude_unit) => {
                self.amplitude_unit = amplitude_unit
            }
            ChartOperation::Interaction(_) => {}
        }

        self.cache.clear();
        self.line_cache.clear();
    }

    pub(crate) fn view<'a>(
        &'a self,
        impulse_response: &'a ImpulseResponse,
        window_settings: &'a WindowSettings,
    ) -> Element<'a, Message> {
        let header = {
            pick_list(
                &data::chart::AmplitudeUnit::ALL[..],
                Some(&self.amplitude_unit),
                |unit| Message::Chart(ChartOperation::AmplitudeUnitChanged(unit)),
            )
        };

        let chart = {
            container(
                chart::impulse_response(
                    &window_settings.window,
                    impulse_response,
                    &self.time_unit,
                    &self.amplitude_unit,
                    self.zoom,
                    self.offset,
                    &self.line_cache,
                )
                .map(ChartOperation::Interaction)
                .map(Message::Chart),
            )
            .style(container::rounded_box)
        };

        let footer = {
            row![container(pick_list(
                &data::chart::TimeSeriesUnit::ALL[..],
                Some(&self.time_unit),
                |unit| Message::Chart(ChartOperation::TimeUnitChanged(unit))
            ))
            .align_right(Length::Fill)]
            .align_y(Alignment::Center)
        };

        container(column![header, chart, footer].spacing(8)).into()
    }

    pub(crate) fn shift_key_released(&mut self) {
        self.shift_key_pressed = false;
    }

    pub(crate) fn shift_key_pressed(&mut self) {
        self.shift_key_pressed = true
    }
}

pub struct WindowSettings {
    pub window: data::Window<data::Samples>,
    pub cache: canvas::Cache,
}

impl WindowSettings {
    pub(crate) fn new(window: data::Window<data::Samples>) -> Self {
        Self {
            window,
            cache: canvas::Cache::new(),
        }
    }
}

pub fn processing_overlay<'a, Message>(
    status: &'a str,
    entry: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    stack([
        container(entry).style(container::bordered_box).into(),
        container(column![text("Computing..."), text(status).size(12)])
            .center(Length::Fill)
            .style(|theme| container::Style {
                border: container::rounded_box(theme).border,
                background: Some(iced::Background::Color(Color::from_rgba(
                    0.0, 0.0, 0.0, 0.8,
                ))),
                ..Default::default()
            })
            .into(),
    ])
    .into()
}
