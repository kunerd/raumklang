use iced::{
    widget::{button, column, container, horizontal_rule, row, scrollable, text},
    Element, Length,
};
use pliced::chart::{line_series, Chart, Labels};
use rustfft::num_complex::ComplexFloat;

use crate::data::{self, impulse_response};

#[derive(Debug, Clone)]
pub enum Message {
    Select(usize),
}

pub enum Action {
    ComputeImpulseResponse(usize),
}

pub struct ImpulseReponses {
    selected: Option<usize>,
}

impl ImpulseReponses {
    pub fn new() -> Self {
        Self { selected: None }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Select(id) => {
                self.selected = Some(id);

                Action::ComputeImpulseResponse(id)
            }
        }
    }

    pub fn view<'a>(&'a self, measurements: &'a [data::Measurement]) -> Element<'a, Message> {
        let sidebar = {
            let header = {
                column!(text("For Measurements"), horizontal_rule(1))
                    .width(Length::Fill)
                    .spacing(5)
            };

            let measurements = measurements.iter().enumerate().map(|(id, entry)| {
                let content = column![text(&entry.name).size(16),]
                    .spacing(5)
                    .clip(true)
                    .spacing(3);

                let style = match self.selected.as_ref() {
                    Some(selected) if *selected == id => button::primary,
                    _ => button::secondary,
                };

                button(content)
                    .on_press_with(move || Message::Select(id))
                    .width(Length::Fill)
                    .style(style)
                    .into()
            });

            container(scrollable(
                column![header, column(measurements).spacing(3)]
                    .spacing(10)
                    .padding(10),
            ))
            .style(container::rounded_box)
        }
        .width(Length::FillPortion(1));

        let content: Element<_> = {
            if let Some(id) = self.selected {
                let state = measurements
                    .get(id)
                    .map(|m| &m.state)
                    .and_then(|s| match s {
                        data::measurement::State::Loaded {
                            impulse_response: impulse_response::State::Computed(impulse_response),
                            ..
                        } => Some(impulse_response),
                        _ => None,
                    });

                match state {
                    Some(impulse_response) => {
                        let chart: Chart<_, (), _> = Chart::new()
                            .width(Length::Fill)
                            .height(Length::Fill)
                            // .x_range(x_scale_fn(-44_10.0, sample_rate)..=x_scale_fn(44_100.0, sample_rate))
                            .y_labels(Labels::default().format(&|v| format!("{v:.2}")))
                            .push_series(
                                line_series(
                                    impulse_response
                                        .data
                                        .iter()
                                        .enumerate()
                                        .map(|(i, s)| (i as f32, s.abs())),
                                )
                                .color(iced::Color::from_rgb8(2, 125, 66)),
                            );
                        chart.into()
                    }
                    // TODO: add spinner
                    None => text("Impulse response not computed, yet.").into(),
                }
            } else {
                text("Please select an entry to view its data.").into()
            }
        };

        row![
            container(sidebar).width(Length::FillPortion(1)),
            container(content).center(Length::FillPortion(4))
        ]
        .into()
    }
}
