use std::fmt;

use crate::{
    icon,
    ui::{self, frequency_response},
};

use iced::{
    widget::{column, container, horizontal_space, row, stack, text, toggler},
    Alignment, Color, Element, Length,
};
use rand::Rng as _;

#[derive(Debug)]
pub struct Item {
    color: iced::Color,
    is_shown: bool,
    state: frequency_response::State,
}

impl Item {
    pub fn from_state(state: frequency_response::State) -> Self {
        let color = random_color();

        Self {
            color,
            is_shown: true,
            state,
        }
    }

    pub fn from_data(frequency_response: raumklang_core::FrequencyResponse) -> Self {
        Self::from_state(frequency_response::State::from_data(frequency_response))
    }

    pub fn view<'a, Message>(
        &'a self,
        measurement_name: &'a str,
        toggle_msg: impl Fn(bool) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: 'a,
    {
        let item = {
            let content = column![
                text(measurement_name).wrapping(text::Wrapping::Glyph),
                row![
                    toggler(self.is_shown)
                        .on_toggle(toggle_msg)
                        .width(Length::Shrink),
                    horizontal_space(),
                    icon::record().color(self.color)
                ]
                .align_y(Alignment::Center)
            ]
            .clip(true)
            .spacing(5)
            .padding(5);

            container(content).style(container::rounded_box)
        };

        match &self.state {
            frequency_response::State::Computing => processing_overlay("Frequency Response", item),
            frequency_response::State::Computed(_frequency_response) => item.into(),
        }
    }
}

#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub enum Smoothing {
    #[default]
    None,
    OneOne,
    OneSecond,
    OneThird,
    OneSixth,
    OneTwelfth,
    OneTwentyFourth,
    OneFourtyEighth,
}

impl Smoothing {
    pub const ALL: [Smoothing; 8] = [
        Smoothing::None,
        Smoothing::OneOne,
        Smoothing::OneSecond,
        Smoothing::OneThird,
        Smoothing::OneSixth,
        Smoothing::OneTwelfth,
        Smoothing::OneTwentyFourth,
        Smoothing::OneFourtyEighth,
    ];

    pub fn fraction(&self) -> Option<u8> {
        match self {
            Smoothing::None => None,
            Smoothing::OneOne => Some(1),
            Smoothing::OneSecond => Some(2),
            Smoothing::OneThird => Some(3),
            Smoothing::OneSixth => Some(6),
            Smoothing::OneTwelfth => Some(12),
            Smoothing::OneTwentyFourth => Some(24),
            Smoothing::OneFourtyEighth => Some(48),
        }
    }
}

impl fmt::Display for Smoothing {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "{} smoothing",
            match self {
                Smoothing::None => "No",
                Smoothing::OneOne => "1/1",
                Smoothing::OneSecond => "1/2",
                Smoothing::OneThird => "1/3",
                Smoothing::OneSixth => "1/6",
                Smoothing::OneTwelfth => "1/12",
                Smoothing::OneTwentyFourth => "1/24",
                Smoothing::OneFourtyEighth => "1/48",
            }
        )
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

fn processing_overlay<'a, Message>(
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
