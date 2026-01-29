use std::fmt::{self, Display};

use crate::ui::impulse_response;
use crate::widget::sidebar;
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
        self.progress = Progress::Finished;
    }

    pub fn view<'a, Message>(
        &'a self,
        measurement_name: &'a str,
        impulse_response_progess: impulse_response::Progress,
        on_toggle: impl Fn(bool) -> Message + 'a,
    ) -> Element<'a, Message>
    where
        Message: Clone + 'a,
    {
        let item = {
            let color_dot = icon::record().color(self.color).align_y(Alignment::Center);

            let content = container(
                text(measurement_name)
                    .size(16)
                    .style(|theme| {
                        let mut base = text::default(theme);

                        let palette = theme.extended_palette();
                        base.color = Some(palette.background.weakest.text);

                        base
                    })
                    .wrapping(text::Wrapping::Glyph)
                    .align_y(Alignment::Center),
            )
            .width(Length::Fill)
            .clip(true);

            let switch =
                container(toggler(self.is_shown).on_toggle(on_toggle)).align_right(Length::Shrink);

            row![color_dot, content, switch]
                .align_y(Alignment::Center)
                .spacing(10)
                .padding(20)
                .into()
        };

        let content = match impulse_response_progess {
            impulse_response::Progress::None | impulse_response::Progress::Computing => {
                processing_overlay("Impulse Response", item)
            }
            impulse_response::Progress::Finished => match self.progress {
                Progress::None | Progress::Computing => {
                    processing_overlay(self.progress.to_string(), item)
                }
                Progress::Finished => item,
            },
        };

        sidebar::item(content, false).into()
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
    Finished,
}

impl Display for Progress {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text = match self {
            Self::None => "Not Started",
            Self::Computing => "Impulse Response",
            Self::Finished => "Finished",
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
