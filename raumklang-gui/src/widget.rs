pub mod meter;
pub mod sidebar;

pub use meter::RmsPeakMeter;

use iced::{
    alignment::Horizontal::Right,
    widget::{column, container, stack, text, text_input, tooltip},
    Color, Element, Font,
    Length::Fill,
};

use std::fmt;

pub fn number_input<'a, E: fmt::Display, Message: Clone + 'a>(
    input: &'a str,
    err: Option<E>,
    msg: impl Fn(String) -> Message + 'a,
) -> Element<'a, Message> {
    let is_err = err.is_some();

    let input = text_input("", input)
        .on_input(msg)
        .font(Font::MONOSPACE)
        .width(5f32.mul_add(10.0, 14.0))
        .size(14)
        .style(move |t, s| {
            let mut base = text_input::default(t, s);

            if is_err {
                let danger = t.extended_palette().danger.strong.color;
                base.border.color = danger;
            }

            base
        })
        .align_x(Right);

    if let Some(err) = err {
        tooltip(
            input,
            text!("{err}").style(text::danger),
            tooltip::Position::Top,
        )
    } else {
        tooltip(input, text(""), tooltip::Position::Top)
    }
    .into()
}

pub fn processing_overlay<'a, Message>(
    status: &'a str,
    entry: impl Into<Element<'a, Message>>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    stack([
        container(entry).style(container::bordered_box).into(),
        container(column![text("Computing..."), text(status).size(12)])
            .center(Fill)
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
