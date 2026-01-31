use iced::{
    widget::{button, column, container, row, space, text},
    Element,
};

#[derive(Debug, Clone)]
pub enum Message {
    Discard,
    Apply,
}

pub fn pending_window() -> Element<'static, Message> {
    container(
        column![
            text("Window pending!").size(18),
            column![
                text("You have modified the window used for frequency response computations."),
                text("You need to discard or apply your changes before proceeding."),
            ]
            .spacing(5),
            row![
                space::horizontal(),
                button("Discard")
                    .style(button::danger)
                    .on_press(Message::Discard),
                button("Apply")
                    .style(button::success)
                    .on_press(Message::Apply)
            ]
            .spacing(5)
        ]
        .spacing(10),
    )
    .padding(20)
    .width(400)
    .style(container::bordered_box)
    .into()
}
