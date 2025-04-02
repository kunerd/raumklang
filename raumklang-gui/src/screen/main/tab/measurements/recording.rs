use iced::{
    widget::{button, column, container, text, Button},
    Color, Element, Length, Subscription,
};

use crate::widgets::colored_circle;

pub struct Recording;

#[derive(Debug, Clone)]
pub enum Message {
    Back,
}

pub enum Action {
    Back,
}

impl Recording {
    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Back => Action::Back,
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content = {
            column![
                text("Recording").size(24),
                button("Cancel").on_press(Message::Back)
            ]
        };

        container(content).width(Length::Fill).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::none()
    }
}

pub fn recording_button<'a, Message: 'a>(msg: Message) -> Button<'a, Message> {
    button(colored_circle(8.0, Color::from_rgb8(200, 56, 42))).on_press(msg)
}
