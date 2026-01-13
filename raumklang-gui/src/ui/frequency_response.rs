use std::fmt::{self, Display};

use crate::ui::impulse_response;
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
    pub progress: Progress,
    pub data: Option<data::FrequencyResponse>,
    pub smoothed: Option<Box<[f32]>>,
}

impl FrequencyResponse {
    pub fn new() -> Self {
        let color = random_color();

        Self {
            color,
            is_shown: true,
            progress: Progress::None,
            data: None,
            smoothed: None,
        }
    }

    pub fn computed(&mut self, data: data::FrequencyResponse) {
        self.data = Some(data);
    }

    pub fn view<'a, Message>(
        &'a self,
        measurement_name: &'a str,
        impulse_response_progess: impulse_response::Progress,
        on_toggle: impl Fn(bool) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: 'a,
    {
        let item = row![
            icon::record().color(self.color).align_y(Alignment::Center),
            container(
                text(measurement_name)
                    .size(16)
                    .style(|theme| {
                        let mut base = text::default(theme);

                        let text_color = theme.extended_palette().background.weakest.text;
                        base.color = Some(text_color);

                        base
                    })
                    .align_y(Alignment::Center)
                    .wrapping(text::Wrapping::Glyph),
            )
            .width(Length::Fill)
            .clip(true),
            container(toggler(self.is_shown).on_toggle(on_toggle)).align_right(Length::Shrink)
        ]
        .align_y(Alignment::Center)
        .spacing(10)
        .padding(6)
        .into();

        if self.data.is_some() {
            item
        } else {
            match impulse_response_progess {
                impulse_response::Progress::None => item,
                impulse_response::Progress::Computing => {
                    processing_overlay("Impulse Response", item)
                }
                impulse_response::Progress::Finished => {
                    processing_overlay(self.progress.to_string(), item)
                }
            }
        }
    }

    pub(crate) fn result(&self) -> Option<&data::FrequencyResponse> {
        self.data.as_ref()
    }
}

impl Default for FrequencyResponse {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum Progress {
    #[default]
    None,
    Computing,
}

impl Display for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::None => "Not Started",
            Self::Computing => "Impulse Response",
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
