use iced::{
    Element, Font,
    Length::Fill,
    widget::{self, Button, Text, container, text},
};

pub fn header<'a, Message: Clone + 'a>(title: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    container(text(title).size(16).font(Font::MONOSPACE).width(Fill))
        .padding(6)
        .into()
}

pub fn button<'a, Message>(title: Text<'a>) -> Button<'a, Message> {
    widget::button(title.size(16).center())
        .width(26)
        .height(26)
        .style(widget::button::secondary)
}

pub fn item<'a, Message: Clone + 'a>(
    content: impl Into<Element<'a, Message>>,
    is_active: bool,
) -> Element<'a, Message> {
    container(content)
        .style(move |theme| {
            container::rounded_box(theme).background(if is_active {
                theme.extended_palette().background.weak.color
            } else {
                theme.extended_palette().background.weakest.color
            })
        })
        .padding([6, 0])
        .into()
}
