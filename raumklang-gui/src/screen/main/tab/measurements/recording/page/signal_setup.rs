use std::fmt::Display;

use crate::data::{
    measurement::{self, config},
    recording::port,
};

use iced::{
    alignment::{Horizontal, Vertical},
    widget::{column, container, horizontal_rule, row, text, text_input},
    Alignment, Element,
};

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

        let duration = config::Duration::from_string(&self.duration);

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
                        field_group(
                            "Frequency",
                            row![
                                number_input("From", &self.start_frequency, range.is_ok())
                                    .unit("Hz")
                                    .on_input(Message::StartFrequency),
                                number_input("To", &self.end_frequency, range.is_ok())
                                    .unit("Hz")
                                    .on_input(Message::EndFrequency)
                            ]
                            .spacing(8)
                            .align_y(Alignment::Center),
                            range.as_ref().err()
                        ),
                        field_group(
                            "Duration",
                            number_input("", &self.duration, duration.is_ok())
                                .unit("s")
                                .on_input(Message::Duration),
                            duration.as_ref().err()
                        )
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
            duration: format!("{}", config.duration().into_inner().as_secs()),
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

fn number_input<'a, Message>(
    label: &'a str,
    value: &'a str,
    is_valid: bool,
) -> NumberInput<'a, Message>
where
    Message: 'a + Clone,
{
    NumberInput::new(label, value, is_valid)
}

struct NumberInput<'a, Message> {
    label: &'a str,
    value: &'a str,
    unit: Option<&'a str>,
    is_valid: bool,
    on_input: Option<Box<dyn Fn(String) -> Message + 'a>>,
}

impl<'a, Message> NumberInput<'a, Message>
where
    Message: 'a + Clone,
{
    fn new(label: &'a str, value: &'a str, is_valid: bool) -> Self {
        Self {
            label,
            value,
            unit: None,
            is_valid,
            on_input: None,
        }
    }

    fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit);
        self
    }

    fn on_input(mut self, on_input: impl Fn(String) -> Message + 'a) -> Self {
        self.on_input = Some(Box::new(on_input));
        self
    }

    fn view(self) -> Element<'a, Message> {
        column![
            text(self.label),
            row![text_input("", self.value)
                .id(text_input::Id::new("from"))
                .align_x(Horizontal::Right)
                .on_input_maybe(self.on_input)
                .style(if self.is_valid {
                    text_input::default
                } else {
                    number_input_danger
                })]
            .push_maybe(self.unit.map(text))
            .align_y(Vertical::Center)
            .spacing(3)
        ]
        .into()
    }
}

fn number_input_danger(theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let danger = theme.extended_palette().danger;

    let mut style = text_input::default(theme, status);

    let color = match status {
        text_input::Status::Active => danger.base.color,
        text_input::Status::Hovered => danger.strong.color,
        text_input::Status::Focused { is_hovered: _ } => danger.strong.color,
        text_input::Status::Disabled => danger.weak.color,
    };

    style.border = style.border.color(color);
    style
}

impl<'a, Message> From<NumberInput<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone,
{
    fn from(number_input: NumberInput<'a, Message>) -> Self {
        number_input.view()
    }
}

fn field_group<'a, Message>(
    label: &'a str,
    content: impl Into<Element<'a, Message>>,
    err: Option<&impl Display>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    container(
        column![text(label), horizontal_rule(1),]
            .push(column!().push_maybe(err.map(|err| {
                text!("{}", err).style(|theme| {
                    let mut style = text::default(theme);
                    style.color = Some(theme.extended_palette().danger.base.color);
                    style
                })
            })))
            .push(content)
            .spacing(6),
    )
    .style(container::rounded_box)
    .padding(8)
    .into()
}
