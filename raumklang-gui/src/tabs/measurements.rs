use std::{fmt::Debug, ops::RangeInclusive, path::PathBuf, sync::Arc};

use iced::{
    keyboard,
    mouse::ScrollDelta,
    widget::{
        self, button, column, container, horizontal_rule, horizontal_space, row, scrollable,
        text::Wrapping,
    },
    Alignment, Element, Length, Point, Subscription, Task,
};
//use pliced::{
//    plotters::line_series,
//    plotters::{Cartesian, Chart},
//};
use pliced::chart::{Axis, Chart, Labels, Margin};
use pliced::series::line_series;

use raumklang_core::WavLoadError;

use crate::{data, delete_icon, OfflineMeasurement};

pub struct Measurements {
    selected: Option<SelectedMeasurement>,

    shift_key_pressed: bool,
    x_max: Option<f32>,
    x_range: RangeInclusive<f32>,
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
    ChartScroll(Option<Point>, Option<ScrollDelta>),
    ShiftKeyPressed,
    ShiftKeyReleased,
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
    pub fn new() -> Self {
        Self {
            selected: None,
            shift_key_pressed: false,
            x_max: Some(10.0),
            x_range: 0.0..=10.0,
        }
    }

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

                self.x_range = signal.map_or(0.0..=10.0, |s| 0.0..=s.duration() as f32);
                self.x_max = Some(*self.x_range.end());
                self.selected = Some(selected);

                (Task::none(), None)
            }
            Message::ChartScroll(pos, scroll_delta) => {
                let Some(pos) = pos else {
                    return (Task::none(), None);
                };

                let Some(ScrollDelta::Lines { y, .. }) = scroll_delta else {
                    return (Task::none(), None);
                };

                match (self.shift_key_pressed, y.is_sign_positive()) {
                    (true, true) => self.scroll_right(),
                    (true, false) => self.scroll_left(),
                    (false, true) => self.zoom_in(pos),
                    (false, false) => self.zoom_out(pos),
                }

                (Task::none(), None)
            }
            Message::ShiftKeyPressed => {
                self.shift_key_pressed = true;
                (Task::none(), None)
            }
            Message::ShiftKeyReleased => {
                self.shift_key_pressed = false;
                (Task::none(), None)
            }
        }
    }

    pub fn view<'a>(
        &'a self,
        loopback: Option<&'a data::MeasurementState<data::Loopback, OfflineMeasurement>>,
        measurements: &'a data::Store<data::Measurement, OfflineMeasurement>,
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
            Some(SelectedMeasurement::Measurement(id)) => {
                measurements.get(id).and_then(|m| match m {
                    data::MeasurementState::Loaded(signal) => Some(signal.data.iter()),
                    data::MeasurementState::NotLoaded(_) => None,
                })
            }
            None => None,
        };

        let content: Element<_> = if let Some(signal) = signal {
            Chart::new()
                .width(Length::Fill)
                .height(Length::Fill)
                .x_range(self.x_range.clone())
                .x_labels(Labels::default().format(&|v| format!("{v:.0}")))
                //.y_labels(Labels::default().format(&|_| "".to_string()))
                .margin(Margin {
                    left: 20.0,
                    ..Default::default()
                })
                .push_series(
                    line_series(signal.enumerate().map(|(i, s)| (i as f32, *s)))
                        .color(iced::Color::from_rgb8(2, 125, 66)),
                )
                .on_scroll(|state| {
                    let pos = state.get_coords();
                    let delta = state.scroll_delta();

                    Message::ChartScroll(pos, delta)
                })
                .into()
        } else {
            widget::text("Please select a measurement.").into()
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

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::batch(vec![
            keyboard::on_key_press(|key, _modifiers| match key {
                keyboard::Key::Named(keyboard::key::Named::Shift) => Some(Message::ShiftKeyPressed),
                _ => None,
            }),
            keyboard::on_key_release(|key, _modifiers| match key {
                keyboard::Key::Named(keyboard::key::Named::Shift) => {
                    Some(Message::ShiftKeyReleased)
                }
                _ => None,
            }),
        ])
    }

    fn scroll_right(&mut self) {
        let old_viewport = self.x_range.clone();
        let length = old_viewport.end() - old_viewport.start();

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = length * SCROLL_FACTOR;

        let mut new_end = old_viewport.end() + offset;
        if let Some(x_max) = self.x_max {
            let viewport_max = x_max + length / 2.0;
            if new_end > viewport_max {
                new_end = viewport_max;
            }
        }

        let new_start = new_end - length;

        self.x_range = new_start..=new_end;
    }

    fn scroll_left(&mut self) {
        let old_viewport = self.x_range.clone();
        let length = old_viewport.end() - old_viewport.start();

        const SCROLL_FACTOR: f32 = 0.2;
        let offset = length * SCROLL_FACTOR;

        let mut new_start = old_viewport.start() - offset;
        let viewport_min = -(length / 2.0);
        if new_start < viewport_min {
            new_start = viewport_min;
        }
        let new_end = new_start + length;

        self.x_range = new_start..=new_end;
    }

    fn zoom_in(&mut self, position: iced::Point) {
        let old_viewport = self.x_range.clone();
        let old_len = old_viewport.end() - old_viewport.start();

        let center_scale: f32 = (position.x - old_viewport.start()) / old_len;

        // FIXME make configurable
        const ZOOM_FACTOR: f32 = 0.8;
        const LOWER_BOUND: f32 = 50.0;
        let mut new_len = old_len * ZOOM_FACTOR;
        if new_len < LOWER_BOUND {
            new_len = LOWER_BOUND;
        }

        let new_start = position.x - (new_len * center_scale);
        let new_end = new_start + new_len;
        self.x_range = new_start..=new_end;
    }

    fn zoom_out(&mut self, position: iced::Point) {
        let old_viewport = self.x_range.clone();
        let old_len = old_viewport.end() - old_viewport.start();

        let center_scale = (position.x - old_viewport.start()) / old_len;

        // FIXME make configurable
        const ZOOM_FACTOR: f32 = 1.2;
        let new_len = old_len * ZOOM_FACTOR;
        //if new_len >= self.max_len {
        //    new_len = self.max_len;
        //}

        let new_start = position.x - (new_len * center_scale);
        let new_end = new_start + new_len;
        self.x_range = new_start..=new_end;
    }
}

fn collecting_list<'a>(
    selected: Option<&SelectedMeasurement>,
    loopback: Option<&'a data::MeasurementState<data::Loopback, OfflineMeasurement>>,
    measurements: &'a data::Store<data::Measurement, OfflineMeasurement>,
) -> Element<'a, Message> {
    let loopback_entry = {
        let content: Element<_> = match &loopback {
            Some(data::MeasurementState::Loaded(signal)) => loopback_list_entry(selected, signal),
            Some(data::MeasurementState::NotLoaded(signal)) => {
                offline_signal_list_entry(signal, Message::RemoveLoopbackMeasurement)
            }
            None => widget::text("Please load a loopback signal.").into(),
        };

        let add_msg = loopback
            .as_ref()
            .map_or(Some(Message::LoadLoopbackMeasurement), |_| None);

        signal_list_category("Loopback", add_msg, content)
    };

    let measurement_entries = {
        let content: Element<_> = {
            if measurements.is_empty() {
                widget::text("Please load a measurement.").into()
            } else {
                let entries: Vec<Element<_>> = measurements
                    .iter()
                    .enumerate()
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
    let header = row!(widget::text(name), horizontal_space()).align_y(Alignment::Center);

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
        widget::text(&signal.name),
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
            widget::text(&signal.0.name),
            horizontal_space(),
            button(delete_icon())
                .on_press(Message::RemoveLoopbackMeasurement)
                .style(button::danger)
        ],
        widget::text(format!("Samples: {}", samples)),
        widget::text(format!("Duration: {} s", samples as f32 / sample_rate)),
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
            widget::text(&signal.name).wrapping(Wrapping::Glyph),
            horizontal_space(),
            button(delete_icon())
                .on_press(Message::RemoveMeasurement(index))
                .style(button::danger)
        ],
        widget::text(format!("Samples: {}", samples)),
        widget::text(format!("Duration: {} s", samples as f32 / sample_rate)),
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
