use iced::{
    widget::{container, text},
    Element, Font,
    Length::Fill,
};

pub fn header<'a, Message: Clone + 'a>(title: impl text::IntoFragment<'a>) -> Element<'a, Message> {
    container(text(title).size(16).font(Font::MONOSPACE).width(Fill))
        .padding(6)
        .into()
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
