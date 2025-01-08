use std::{io::ErrorKind, path::PathBuf};

use iced::{
    widget::{button, column, container, horizontal_rule, horizontal_space, row, scrollable, text},
    Alignment, Element, Length, Task,
};
use thiserror::Error;

use crate::{model, widgets::chart::{self, SignalChart}};
use crate::MeasurementState;

#[derive(Default)]
pub struct Measurements {
    selected: Option<SelectedMeasurement>,
    chart: Option<SignalChart>,
}

#[derive(Debug, Clone)]
pub enum SelectedMeasurement {
    Loopback,
    Measurement(usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadMeasurement,
    LoadLoopbackMeasurement,
    MeasurementSelected(SelectedMeasurement),
    TimeSeriesChart(chart::SignalChartMessage),
}

#[derive(Debug, Clone)]
pub enum Event {
    LoadLoopbackMeasurement,
    LoadMeasurement,
}

#[derive(Debug, Clone)]
pub enum Error {
    File(WavLoadError),
    DialogClosed,
}

#[derive(Error, Debug, Clone)]
pub enum WavLoadError {
    #[error("couldn't read file")]
    IoError(PathBuf, ErrorKind),
    #[error("unknown")]
    Other,
}

impl Measurements {
    pub fn view<'a>(&'a self, data: &'a crate::MeasurementsState) -> Element<'a, Message> {
        let (loopback, measurements) = match data {
            crate::MeasurementsState::Collecting {
                loopback,
                measurements,
            } => (loopback.as_ref(), measurements),
            crate::MeasurementsState::Analysing(data) => (Some(&data.loopback), &data.measurements),
        };

        let side_menu: Element<_> = {
            let loopback_entry = {
                let content: Element<_> = match loopback {
                    Some(MeasurementState::Loaded(signal)) => {
                        let style = if let Some(SelectedMeasurement::Loopback) = self.selected {
                            button::primary
                        } else {
                            button::secondary
                        };

                        button(signal_list_entry(signal))
                            .on_press(Message::MeasurementSelected(SelectedMeasurement::Loopback))
                            .style(style)
                            .width(Length::Fill)
                            .into()
                    }
                    Some(MeasurementState::NotLoaded(signal)) => offline_signal_list_entry(signal),
                    None => text("Please load a loopback signal.").into(),
                };

                let add_msg = loopback
                    .as_ref()
                    .map_or(Some(Message::LoadLoopbackMeasurement), |_| None);

                signal_list_category("Loopback", add_msg, content)
            };

            let measurement_entries = {
                let content: Element<_> = {
                    if measurements.is_empty() {
                        text("Please load a measurement.").into()
                    } else {
                        let entries: Vec<Element<_>> = measurements
                            .iter()
                            .enumerate()
                            .map(|(index, state)| match state {
                                MeasurementState::Loaded(signal) => {
                                    let style = match self.selected {
                                        Some(SelectedMeasurement::Measurement(i)) if i == index => {
                                            button::primary
                                        }
                                        Some(_) => button::secondary,
                                        None => button::secondary,
                                    };
                                    button(signal_list_entry(signal))
                                        .on_press(Message::MeasurementSelected(
                                            SelectedMeasurement::Measurement(index),
                                        ))
                                        .width(Length::Fill)
                                        .style(style)
                                        .into()
                                }
                                MeasurementState::NotLoaded(signal) => {
                                    offline_signal_list_entry(signal)
                                }
                            })
                            .collect();

                        column(entries).padding(5).spacing(5).into()
                    }
                };

                signal_list_category("Measurements", Some(Message::LoadMeasurement), content)
            };
            container(column!(loopback_entry, measurement_entries).spacing(10))
                .padding(5)
                .into()
        };

        let content = if let Some(chart) = &self.chart {
            chart.view().map(Message::TimeSeriesChart)
        } else {
            text("Please select a measurement.").into()
        };

        let side_menu = scrollable(side_menu);
        row!(
            side_menu.width(Length::FillPortion(1)),
            container(content).width(Length::FillPortion(3))
        )
        .into()
    }

    pub fn update(
        &mut self,
        msg: Message,
        data: &crate::MeasurementsState,
    ) -> (Task<Message>, Option<Event>) {
        match msg {
            Message::LoadLoopbackMeasurement => {
                (Task::none(), Some(Event::LoadLoopbackMeasurement))
            }
            Message::LoadMeasurement => (Task::none(), Some(Event::LoadMeasurement)),
            Message::MeasurementSelected(selected) => {
                let signal = match selected {
                    SelectedMeasurement::Loopback => data.loopback(),
                    SelectedMeasurement::Measurement(id) => {
                        let measurements = data.measurements();
                        measurements.get(id)
                    }
                };
                self.selected = Some(selected);

                self.chart = match signal {
                    Some(MeasurementState::Loaded(m)) => Some(chart::SignalChart::new(
                        m.clone(),
                        chart::TimeSeriesUnit::Time,
                    )),
                    _ => None,
                };

                (Task::none(), None)
            }
            Message::TimeSeriesChart(msg) => {
                if let Some(chart) = &mut self.chart {
                    chart.update_msg(msg);
                }
                (Task::none(), None)
            }
        }
    }
}

fn signal_list_category<'a>(
    name: &'a str,
    add_msg: Option<Message>,
    content: Element<'a, Message>,
) -> Element<'a, Message> {
    let header = row!(text(name), horizontal_space()).align_y(Alignment::Center);

    let header = if let Some(msg) = add_msg {
        header.push(button("+").on_press(msg))
    } else {
        header
    };

    column!(header, horizontal_rule(1), content)
        .width(Length::Fill)
        .spacing(5)
        .padding(10)
        .into()
}

fn offline_signal_list_entry(signal: &crate::OfflineMeasurement) -> Element<'_, Message> {
    column!(text(&signal.name), button("Reload"))
        .padding(2)
        .into()
}

fn signal_list_entry(signal: &model::Measurement) -> Element<'_, Message> {
    let samples = signal.data.len();
    let sample_rate = signal.sample_rate as f32;
    column!(
        text(&signal.name),
        text(format!("Samples: {}", samples)),
        text(format!("Duration: {} s", samples as f32 / sample_rate)),
    )
    .padding(2)
    .into()
}
