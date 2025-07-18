use std::{
    fmt::{self, Display},
    ops::RangeInclusive,
};

use crate::{
    data, icon,
    ui::{self, frequency_response, impulse_response},
};

use iced::{
    widget::{canvas, column, container, row, stack, text, text::IntoFragment, toggler},
    Alignment, Color, Element, Length,
};
use rand::Rng as _;

#[derive(Debug)]
pub struct Item {
    pub color: iced::Color,
    pub is_shown: bool,
    pub state: State,
    pub smoothed: Option<Box<[f32]>>,
}

impl Item {
    fn new(state: State) -> Self {
        let color = random_color();

        Self {
            color,
            is_shown: true,
            state,
            smoothed: None,
        }
    }

    pub fn from_impulse_response_state(state: impulse_response::State) -> Self {
        let state = match state {
            impulse_response::State::Computing => State::ComputingImpulseResponse,
            impulse_response::State::Computed(_) => State::ComputingFrequencyResponse,
        };

        Self::new(state)
    }

    pub fn from_state(state: frequency_response::State) -> Self {
        let state = match state {
            frequency_response::State::Computing => State::ComputingFrequencyResponse,
            frequency_response::State::Computed(frequency_response) => {
                State::Computed(frequency_response)
            }
        };

        Self::new(state)
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
                row![
                    icon::record().color(self.color).align_y(Alignment::Center),
                    text(measurement_name)
                        .align_y(Alignment::Center)
                        .wrapping(text::Wrapping::Glyph),
                ]
                .align_y(Alignment::Center)
                .spacing(8),
                container(
                    toggler(self.is_shown)
                        .on_toggle(toggle_msg)
                        .width(Length::Shrink)
                )
                .align_right(Length::Fill)
            ]
            .spacing(8);

            container(content).style(container::rounded_box)
        };

        match &self.state {
            State::Computed(_) => item.into(),
            state => processing_overlay(state.to_string(), item),
        }
    }
}

#[derive(Debug)]
pub enum State {
    ComputingImpulseResponse,
    ComputingFrequencyResponse,
    Computed(ui::FrequencyResponse),
}

impl Display for State {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            State::ComputingImpulseResponse => "Impulse Response",
            State::ComputingFrequencyResponse => "Frequency Response",
            State::Computed(_) => "Computed",
        };

        write!(f, "{}", text)
    }
}

#[derive(Debug, Default)]
pub struct ChartData {
    pub x_max: Option<f32>,
    pub x_range: Option<RangeInclusive<f32>>,
    pub cache: canvas::Cache,
    pub shift_key_pressed: bool,
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

pub async fn smooth_frequency_response(
    id: ui::measurement::Id,
    frequency_response: ui::FrequencyResponse,
    fraction: u8,
) -> (ui::measurement::Id, Box<[f32]>) {
    let data = tokio::task::spawn_blocking(move || {
        data::smooth_fractional_octave(&frequency_response.data.clone(), fraction)
    })
    .await
    .unwrap()
    .into_boxed_slice();

    (id, data)
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
    status: impl IntoFragment<'a>,
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
