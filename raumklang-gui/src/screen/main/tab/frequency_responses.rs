use iced::{
    widget::{column, container, horizontal_space, row, stack, text, toggler},
    Alignment, Color, Element,
    Length::{self, FillPortion},
};
use pliced::chart::{line_series, Chart};
use rand::Rng;
use raumklang_core::dbfs;

use crate::{
    data::{self, frequency_response, measurement},
    widgets::colored_circle,
};

pub struct FrequencyResponses {
    entries: Vec<Entry>,
}

#[derive(Debug, Clone)]
pub enum Message {
    ShowInGraphToggled(usize, bool),
}

struct Entry {
    measurement_id: usize,
    show: bool,
    color: iced::Color,
}

impl FrequencyResponses {
    pub fn new(iter: impl Iterator<Item = usize>) -> Self {
        let entries = iter.map(Entry::new).collect();

        Self { entries }
    }

    pub fn view<'a>(&'a self, measurements: &[&'a data::Measurement]) -> Element<'a, Message> {
        let sidebar = {
            let entries = self.entries.iter().enumerate().flat_map(
                |(id, entry)| -> Option<Element<Message>> {
                    let measurement = measurements.get(entry.measurement_id)?;

                    let name = &measurement.details.name;
                    let state = &measurement.analysis;

                    entry.view(id, name, state).into()
                },
            );

            container(column(entries).spacing(10).padding(8)).style(container::rounded_box)
        };

        let content: Element<_> = if self.entries.iter().any(|entry| entry.show) {
            let series_list = self
                .entries
                .iter()
                .flat_map(|entry| {
                    if entry.show {
                        let measurement = measurements.get(entry.measurement_id)?;
                        let frequency_response = measurement.frequency_response()?;

                        Some((frequency_response, entry.color))
                    } else {
                        None
                    }
                })
                .map(|(frequency_response, color)| {
                    line_series(
                        frequency_response
                            .origin
                            .data
                            .iter()
                            .enumerate()
                            .map(|(i, s)| (i as f32, dbfs(s.re.abs()))),
                    )
                    .color(color)
                });

            let chart: Chart<Message, ()> = Chart::new()
                .x_range(0.0..=1000.0)
                .extend_series(series_list);

            chart.into()
        } else {
            text("Please select a frequency respone.").into()
        };

        row![
            container(sidebar).width(FillPortion(1)),
            container(content).center(Length::FillPortion(4))
        ]
        .into()
    }

    pub fn update(&mut self, message: Message) {
        match message {
            Message::ShowInGraphToggled(id, state) => {
                if let Some(entry) = self.entries.get_mut(id) {
                    entry.show = state;
                }
            }
        }
    }
}

impl Entry {
    pub fn new(id: usize) -> Self {
        Self {
            measurement_id: id,
            show: true,
            color: random_color(),
        }
    }

    pub fn view<'a>(
        &'a self,
        id: usize,
        name: &'a str,
        state: &'a measurement::Analysis,
    ) -> Element<'a, Message> {
        let entry = {
            let content = column![
                text(name).wrapping(text::Wrapping::Glyph),
                row![
                    toggler(self.show)
                        .on_toggle(move |state| Message::ShowInGraphToggled(id, state))
                        .width(Length::Shrink),
                    horizontal_space(),
                    colored_circle(10.0, self.color),
                ]
                .align_y(Alignment::Center)
            ]
            .clip(true)
            .spacing(5)
            .padding(5);

            container(content).style(container::rounded_box)
        };

        match state {
            measurement::Analysis::None => panic!(),
            measurement::Analysis::ImpulseResponse(_) => {
                processing_overlay("Impulse Response", entry).into()
            }
            measurement::Analysis::FrequencyResponse(_, frequency_response::State::Computing) => {
                processing_overlay("Frequency Response", entry).into()
            }
            measurement::Analysis::FrequencyResponse(_, frequency_response::State::Computed(_)) => {
                entry.into()
            }
        }
    }
}

fn random_color() -> iced::Color {
    const MAX_COLOR_VALUE: u8 = 255;

    // TODO: replace with color palette
    let red = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let green = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);
    let blue = rand::thread_rng().gen_range(0..MAX_COLOR_VALUE);

    iced::Color::from_rgb8(red, green, blue)
}

fn processing_overlay<'a>(
    status: &'a str,
    entry: impl Into<Element<'a, Message>>,
) -> Element<'a, Message> {
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
