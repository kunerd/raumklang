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
    pub zoom: chart::Zoom,
    pub offset: i64,
    pub data_cache: canvas::Cache,
    pub overlay_cache: canvas::Cache,
}

impl Chart {
    pub(crate) fn update(&mut self, chart_operation: ChartOperation) {
        match chart_operation {
            ChartOperation::TimeUnitChanged(time_unit) => {
                self.time_unit = time_unit;
                self.data_cache.clear();
                self.overlay_cache.clear();
            }
            ChartOperation::AmplitudeUnitChanged(amplitude_unit) => {
                self.amplitude_unit = amplitude_unit;
                self.data_cache.clear();
                self.overlay_cache.clear();
            }
            ChartOperation::Interaction(_) => {}
        }
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
                    &self.data_cache,
                    &self.overlay_cache,
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
}

impl WindowSettings {
    pub(crate) fn new(window: data::Window<data::Samples>) -> Self {
        Self { window }
    }
}
