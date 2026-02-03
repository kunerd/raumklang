use crate::widget::sidebar;
use crate::{data, icon};

use iced::Alignment;
use iced::widget::stack;
use iced::widget::text::IntoFragment;
use iced::{
    Element, Length,
    widget::{column, container, row, text, toggler},
};

use rand::Rng as _;

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub color: iced::Color,
    pub is_shown: bool,

    pub smoothed: Option<Box<[f32]>>,

    pub state: State,
}

#[derive(Debug, Clone)]
pub enum State {
    None,
    WaitingForImpulseResponse,
    Computing,
    Computed(data::FrequencyResponse),
}

impl FrequencyResponse {
    pub fn new() -> Self {
        let color = random_color();

        Self {
            color,
            is_shown: true,
            smoothed: None,

            state: State::None,
        }
    }

    pub fn view<'a, Message>(
        &'a self,
        measurement_name: &'a str,
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

        let content = match self.state {
            State::None => item,
            State::WaitingForImpulseResponse => processing_overlay("Impulse Response", item),
            State::Computing => processing_overlay("Computing ...", item),
            State::Computed(_) => item,
        };

        sidebar::item(content, false)
    }

    pub fn result(&self) -> Option<&data::FrequencyResponse> {
        let State::Computed(ref result) = self.state else {
            return None;
        };

        Some(result)
    }

    pub fn set_result(&mut self, fr: data::FrequencyResponse) {
        self.state = State::Computed(fr)
    }
}

impl Default for FrequencyResponse {
    fn default() -> Self {
        Self::new()
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
