use iced::{
    widget::{column, pick_list, row, text, text_input},
    Element,
};
use iced_aw::number_input;

use raumklang_core::{Window, WindowBuilder};

#[derive(Debug, Clone)]
pub enum Message {
    LeftTypeChanged(Window),
    RightTypeChanged(Window),
    LeftWidthChanged(usize),
    RightWidthChanged(usize),
    MidWidthChanged(usize),
}

#[derive(Debug)]
pub struct WindowSettings {
    pub window_builder: WindowBuilder,
    max_width: usize,
}

impl WindowSettings {
    pub fn new(max_width: usize) -> Self {
        let half_width = max_width / 2;
        let mut window_builder =
            WindowBuilder::new(Window::Hann, half_width, Window::Hann, half_width);
        window_builder.offset = 1;
        Self {
            window_builder,
            max_width,
        }
    }

    pub fn view(&self) -> Element<Message> {
        let left_window_settings = maybe_window_settings_view(&self.window_builder.left_side);
        let right_window_settings = maybe_window_settings_view(&self.window_builder.right_side);

        column![
            row![
                text("Left hand window:"),
                pick_list(
                    Window::ALL,
                    Some(self.window_builder.left_side),
                    Message::LeftTypeChanged
                ),
                text("width:"),
                number_input(
                    &self.window_builder.left_side_width,
                    0..self.max_width,
                    Message::LeftWidthChanged
                )
            ]
            .push_maybe(left_window_settings),
            row![
                text("Right hand window:"),
                pick_list(
                    Window::ALL,
                    Some(self.window_builder.right_side),
                    Message::RightTypeChanged
                ),
                text("width:"),
                number_input(
                    &self.window_builder.right_side_width,
                    0..self.max_width,
                    Message::RightWidthChanged
                )
            ]
            .push_maybe(right_window_settings),
            row![
                text("Window offset"),
                number_input(
                    &self.window_builder.offset,
                    0..self.max_width,
                    Message::MidWidthChanged
                )
            ]
        ]
        .into()
    }

    pub fn update(&mut self, msg: Message) {
        match msg {
            Message::LeftTypeChanged(window) => self.window_builder.left_side = window,
            Message::RightTypeChanged(window) => self.window_builder.right_side = window,
            Message::LeftWidthChanged(width) => self.window_builder.left_side_width = width,
            Message::MidWidthChanged(width) => self.window_builder.offset = width,
            Message::RightWidthChanged(width) => self.window_builder.right_side_width = width,
        };
    }
}

fn maybe_window_settings_view(window: &Window) -> Option<Element<'static, Message>> {
    match window {
        Window::Hann => None,
        Window::Tukey(alpha) => Some(text_input("alpha", format!("{alpha}").as_str())),
    }
    .map(|v| v.into())
}
