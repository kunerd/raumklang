use crate::data::{
    measurement::{self, config},
    recording::port,
};

use iced::{
    widget::{column, container, horizontal_rule, horizontal_space, row, text, text_input},
    Alignment,
};

use std::time::Duration;

#[derive(Debug)]
pub struct SignalSetup {
    duration: String,
    start_frequency: String,
    end_frequency: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Message {
    StartFrequency(String),
    EndFrequency(String),
    Duration(String),
    Next(config::Config),
}

impl SignalSetup {
    pub fn new() -> Self {
        measurement::Config::default().into()
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Option<measurement::Config> {
        match message {
            Message::StartFrequency(freq) => {
                self.start_frequency = freq;
                None
            }
            Message::EndFrequency(freq) => {
                self.end_frequency = freq;
                None
            }
            Message::Duration(duration) => {
                self.duration = duration;
                None
            }
            Message::Next(config) => Some(config),
        }
    }

    pub fn view(&self, config: &port::Config) -> super::Component<Message> {
        let range =
            config::FrequencyRange::from_strings(&self.start_frequency, &self.end_frequency);

        let range_err = range.is_err();

        let duration = self.duration.parse().map(Duration::from_secs_f32);

        let duration_err = duration.is_err();

        super::Component::new("Signal Setup")
            .content(
                column![
                    row![
                        column![
                            text("Out port"),
                            container(text!("{}", config.out_port()))
                                .padding(3)
                                .style(container::rounded_box)
                        ]
                        .spacing(6),
                        column![
                            text("In port"),
                            container(text!("{}", config.in_port())).padding(3)
                        ]
                        .spacing(6),
                    ]
                    .spacing(12),
                    row![
                        {
                            let color = move |theme: &iced::Theme| {
                                if range_err {
                                    theme.extended_palette().danger.weak.color
                                } else {
                                    theme.extended_palette().secondary.strong.color
                                }
                            };

                            container(
                                column![text("Frequency"), horizontal_rule(1),]
                                    .push_maybe(range.as_ref().err().map(|err| text!("{err}")))
                                    .push(
                                        row![
                                            text("From"),
                                            text_input("From", &self.start_frequency)
                                                .on_input(Message::StartFrequency)
                                                .style(move |theme, status| {
                                                    let mut style =
                                                        text_input::default(theme, status);
                                                    style.border = style.border.color(color(theme));
                                                    style
                                                }),
                                            text("To"),
                                            text_input("To", &self.end_frequency)
                                                .on_input(Message::EndFrequency),
                                        ]
                                        .spacing(8)
                                        .align_y(Alignment::Center),
                                    )
                                    .spacing(6),
                            )
                            .style(move |theme| {
                                let style = container::rounded_box(theme);
                                if range_err {
                                    style.color(color(theme))
                                } else {
                                    style
                                }
                            })
                            .padding(8)
                        },
                        container(row![
                            column![
                                text("Duration"),
                                horizontal_rule(1),
                                text_input("Duration", &self.duration)
                                    .on_input(Message::Duration)
                                    .style(move |theme: &iced::Theme, status| {
                                        if duration_err {
                                            text_input::Style {
                                                border: iced::Border {
                                                    color: theme
                                                        .extended_palette()
                                                        .danger
                                                        .base
                                                        .color,
                                                    width: 1.0,
                                                    ..Default::default()
                                                },
                                                ..text_input::default(theme, status)
                                            }
                                        } else {
                                            text_input::default(theme, status)
                                        }
                                    }),
                            ]
                            .spacing(8),
                            horizontal_space()
                        ])
                        .style(move |theme| {
                            let style = container::rounded_box(theme);
                            if duration_err {
                                style.color(theme.extended_palette().danger.strong.color)
                            } else {
                                style
                            }
                        })
                        .padding(8)
                    ]
                    .spacing(8)
                    .align_y(Alignment::Center),
                ]
                .spacing(12),
            )
            .next_button("Next", {
                if let (Ok(range), Ok(duration)) = (range, duration) {
                    let config = measurement::Config::new(range, duration);
                    Some(Message::Next(config))
                } else {
                    None
                }
            })
    }
}

impl From<&measurement::Config> for SignalSetup {
    fn from(config: &measurement::Config) -> Self {
        Self {
            duration: format!("{}", config.duration().as_secs()),
            start_frequency: format!("{}", config.start_frequency()),
            end_frequency: format!("{}", config.end_frequency()),
        }
    }
}

impl From<measurement::Config> for SignalSetup {
    fn from(config: measurement::Config) -> Self {
        SignalSetup::from(&config)
    }
}
