use std::{ops::Range, path::PathBuf, sync::Arc};

use iced::{
    widget::{
        button, column, container, horizontal_rule, horizontal_space, row, scrollable, text,
        text::Wrapping,
    },
    Alignment, Element, Length, Task,
};
use pliced::widget::line_series;
use raumklang_core::WavLoadError;

use crate::{
    data::{self},
    delete_icon,
    widgets::charts::{self, measurement},
    OfflineMeasurement,
};

#[derive(Default)]
pub struct Measurements {
    selected: Option<SelectedMeasurement>,
    chart: Option<measurement::SignalChart>,
}

#[derive(Debug, Clone)]
pub enum SelectedMeasurement {
    Loopback,
    Measurement(usize),
}

#[derive(Debug, Clone)]
pub enum Message {
    LoadMeasurement,
    RemoveMeasurement(usize),
    LoadLoopbackMeasurement,
    RemoveLoopbackMeasurement,
    MeasurementSelected(SelectedMeasurement),
    TimeSeriesChart(measurement::Message),
}

#[derive(Debug, Clone)]
pub enum Event {
    Load,
    Remove(usize, data::MeasurementId),
    LoadLoopback,
    RemoveLoopback,
}

#[derive(Debug, Clone)]
pub enum Error {
    File(PathBuf, Arc<WavLoadError>),
    DialogClosed,
}

impl Measurements {
    pub fn update(
        &mut self,
        msg: Message,
        loopback: Option<&data::Loopback>,
        measurements: &data::Store<data::Measurement, OfflineMeasurement>,
    ) -> (Task<Message>, Option<Event>) {
        match msg {
            Message::LoadLoopbackMeasurement => (Task::none(), Some(Event::LoadLoopback)),
            Message::RemoveLoopbackMeasurement => (Task::none(), Some(Event::RemoveLoopback)),
            Message::LoadMeasurement => (Task::none(), Some(Event::Load)),
            Message::RemoveMeasurement(index) => {
                let event = measurements
                    .get_loaded_id(index)
                    .map(|id| Event::Remove(index, id));

                (Task::none(), event)
            }
            Message::MeasurementSelected(selected) => {
                let signal = match selected {
                    SelectedMeasurement::Loopback => {
                        loopback.map(|l| raumklang_core::Measurement::from(l.0.data.clone()))
                    }
                    SelectedMeasurement::Measurement(id) => {
                        measurements.get(id).and_then(|m| match m {
                            data::MeasurementState::Loaded(m) => Some(m.data.clone()),
                            data::MeasurementState::NotLoaded(_) => None,
                        })
                    }
                };
                self.selected = Some(selected);

                self.chart = signal.as_ref().map(|signal| {
                    measurement::SignalChart::new(signal, charts::TimeSeriesUnit::Time)
                });

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

    pub fn view<'a>(
        &'a self,
        loopback: Option<&'a data::MeasurementState<data::Loopback, OfflineMeasurement>>,
        measurements: impl Iterator<
            Item = (
                usize,
                data::MeasurementState<&'a data::Measurement, &'a OfflineMeasurement>,
            ),
        >,
    ) -> Element<'a, Message> {
        let measurements_list = collecting_list(self.selected.as_ref(), loopback, measurements);
        let side_menu =
            container(container(scrollable(measurements_list).height(Length::Fill)).padding(8))
                .style(container::rounded_box);

        let signal = match self.selected {
            Some(SelectedMeasurement::Loopback) => loopback.and_then(|l| match l {
                data::MeasurementState::Loaded(m) => Some(m.0.data.0.iter()),
                data::MeasurementState::NotLoaded(_) => None,
            }),
            Some(SelectedMeasurement::Measurement(_id)) => None,
            None => None,
        };
        let content: Element<_> = if let Some(signal) = signal {
            pliced::widget::Chart::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .push_series(
                    line_series(signal.enumerate().map(|(i, s)| (i as f32, *s)))
                        .color(iced::Color::from_rgba(100.0, 150.0, 0.0, 0.8).into()),
                )
                .into()
        } else {
            text("Please select a measurement.").into()
        };

        row!(
            side_menu.width(Length::FillPortion(1)),
            container(content)
                .center(Length::FillPortion(4))
                .width(Length::FillPortion(4))
        )
        .height(Length::Fill)
        .spacing(5)
        .into()
    }
}

fn collecting_list<'a>(
    selected: Option<&SelectedMeasurement>,
    loopback: Option<&'a data::MeasurementState<data::Loopback, OfflineMeasurement>>,
    measurements: impl Iterator<
        Item = (
            usize,
            data::MeasurementState<&'a data::Measurement, &'a OfflineMeasurement>,
        ),
    >,
) -> Element<'a, Message> {
    let loopback_entry = {
        let content: Element<_> = match &loopback {
            Some(data::MeasurementState::Loaded(signal)) => loopback_list_entry(selected, signal),
            Some(data::MeasurementState::NotLoaded(signal)) => {
                offline_signal_list_entry(signal, Message::RemoveLoopbackMeasurement)
            }
            None => text("Please load a loopback signal.").into(),
        };

        let add_msg = loopback
            .as_ref()
            .map_or(Some(Message::LoadLoopbackMeasurement), |_| None);

        signal_list_category("Loopback", add_msg, content)
    };

    let measurements: Vec<_> = measurements.collect();
    let measurement_entries = {
        let content: Element<_> = {
            if measurements.is_empty() {
                text("Please load a measurement.").into()
            } else {
                let entries: Vec<Element<_>> = measurements
                    .into_iter()
                    .map(|(index, state)| match state {
                        data::MeasurementState::Loaded(signal) => {
                            measurement_list_entry(selected, signal, index)
                        }
                        data::MeasurementState::NotLoaded(signal) => {
                            offline_signal_list_entry(signal, Message::RemoveMeasurement(index))
                        }
                    })
                    .collect();

                column(entries).spacing(5).into()
            }
        };

        signal_list_category("Measurements", Some(Message::LoadMeasurement), content)
    };

    column!(loopback_entry, measurement_entries)
        .spacing(10)
        .into()
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
        .into()
}

fn offline_signal_list_entry(
    signal: &crate::OfflineMeasurement,
    delete_msg: Message,
) -> Element<'_, Message> {
    column!(row![
        text(&signal.name),
        horizontal_space(),
        button(delete_icon())
            .on_press(delete_msg)
            .style(button::danger)
    ],)
    .into()
}

fn loopback_list_entry<'a>(
    selected: Option<&SelectedMeasurement>,
    signal: &'a data::Loopback,
) -> Element<'a, Message> {
    let samples = signal.0.data.0.duration();
    let sample_rate = signal.0.data.0.sample_rate() as f32;
    let content = column!(
        row![
            text(&signal.0.name),
            horizontal_space(),
            button(delete_icon())
                .on_press(Message::RemoveLoopbackMeasurement)
                .style(button::danger)
        ],
        text(format!("Samples: {}", samples)),
        text(format!("Duration: {} s", samples as f32 / sample_rate)),
    );

    let style = if let Some(SelectedMeasurement::Loopback) = selected {
        button::primary
    } else {
        button::secondary
    };

    button(content)
        .on_press(Message::MeasurementSelected(SelectedMeasurement::Loopback))
        .style(style)
        .width(Length::Fill)
        .into()
}

fn measurement_list_entry<'a>(
    selected: Option<&SelectedMeasurement>,
    signal: &'a data::Measurement,
    index: usize,
) -> Element<'a, Message> {
    let samples = signal.data.duration();
    let sample_rate = signal.data.sample_rate() as f32;
    let content = column!(
        row![
            text(&signal.name).wrapping(Wrapping::Glyph),
            horizontal_space(),
            button(delete_icon())
                .on_press(Message::RemoveMeasurement(index))
                .style(button::danger)
        ],
        text(format!("Samples: {}", samples)),
        text(format!("Duration: {} s", samples as f32 / sample_rate)),
    );

    let style = match selected {
        Some(SelectedMeasurement::Measurement(selected)) if *selected == index => button::primary,
        Some(_) => button::secondary,
        None => button::secondary,
    };

    button(content)
        .on_press(Message::MeasurementSelected(
            SelectedMeasurement::Measurement(index),
        ))
        .width(Length::Fill)
        .style(style)
        .into()
}
