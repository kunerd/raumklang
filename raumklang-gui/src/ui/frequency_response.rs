use std::fmt::{self, Display};

use crate::{data, icon};

use iced::widget::stack;
use iced::widget::text::IntoFragment;
use iced::Alignment;
use iced::{
    widget::{column, container, row, text, toggler},
    Element, Length,
};

use rand::Rng as _;

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub color: iced::Color,
    pub is_shown: bool,
    pub state: State,
    pub smoothed: Option<Box<[f32]>>,
}

impl FrequencyResponse {
    pub fn new(state: State) -> Self {
        let color = random_color();

        Self {
            color,
            is_shown: true,
            state,
            smoothed: None,
        }
    }

    pub fn computed(&self) -> Option<&data::FrequencyResponse> {
        if let State::Computed(ref frequency_response) = self.state {
            Some(frequency_response)
        } else {
            None
        }
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

    pub(crate) fn apply(&mut self, event: data::frequency_response::Event) {
        dbg!("Frequency response computing started.");
        match event {
            data::frequency_response::Event::ComputingStarted => {
                self.state = State::ComputingImpulseResponse
            }
        }
    }
}

impl Default for FrequencyResponse {
    fn default() -> Self {
        Self::new(State::ComputingImpulseResponse)
    }
}

#[derive(Debug, Clone)]
pub enum State {
    ComputingImpulseResponse,
    ComputingFrequencyResponse,
    Computed(data::FrequencyResponse),
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
                background: Some(iced::Background::Color(iced::Color::from_rgba(
                    0.0, 0.0, 0.0, 0.8,
                ))),
                ..Default::default()
            })
            .into(),
    ])
    .into()
}
