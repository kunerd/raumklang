use crate::{
    data::spectral_decay::{self, Shift, WindowWidth},
    icon,
    widget::number_input,
};

use iced::{
    widget::{button, column, container, row, rule, scrollable, space, text},
    Alignment::Center,
    Element,
};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Apply(spectral_decay::Config),
    Discard,
    ShiftChanged(String),
    LeftWidthChanged(String),
    RightWidthChanged(String),
}

pub(crate) enum Action {
    Apply(spectral_decay::Config),
    Discard,
}

#[derive(Debug)]
pub(crate) struct Config {
    shift: String,
    left_window_width: String,
    right_window_width: String,
    original_config: spectral_decay::Config,
}

impl Config {
    pub(crate) fn new(config: spectral_decay::Config) -> Self {
        Self {
            shift: config.shift.as_millis().to_string(),
            left_window_width: config.left_window_width.as_millis().to_string(),
            right_window_width: config.right_window_width.as_millis().to_string(),
            original_config: config,
        }
    }

    pub(crate) fn update(&mut self, message: Message) -> Option<Action> {
        match message {
            Message::Apply(config) => Some(Action::Apply(config)),
            Message::Discard => Some(Action::Discard),
            Message::ShiftChanged(shift) => {
                self.shift = shift;

                None
            }
            Message::LeftWidthChanged(left_width) => {
                self.left_window_width = left_width;

                None
            }
            Message::RightWidthChanged(right_width) => {
                self.right_window_width = right_width;
                None
            }
        }
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        let shift = Shift::from_millis_string(&self.shift);
        let left_window_width = WindowWidth::from_millis_string(&self.left_window_width);
        let right_window_width = WindowWidth::from_millis_string(&self.right_window_width);

        let config = if let (Ok(shift), Ok(left_window_width), Ok(right_window_width)) = (
            shift.as_ref(),
            left_window_width.as_ref(),
            right_window_width.as_ref(),
        ) {
            let new_config = spectral_decay::Config {
                shift: *shift,
                left_window_width: *left_window_width,
                right_window_width: *right_window_width,
                // TODO make configurable
                smoothing_fraction: 24,
            };

            if new_config != self.original_config {
                Some(new_config)
            } else {
                None
            }
        } else {
            None
        };

        container(scrollable(
            column![
                row![
                    text("Spectral Decay Config").size(18),
                    space::horizontal(),
                    button(icon::reset().center()).style(button::secondary)
                ],
                rule::horizontal(1),
                column![
                    row![
                        "Shift",
                        space::horizontal(),
                        number_input(&self.shift, shift.as_ref().err(), Message::ShiftChanged),
                        " ms"
                    ]
                    .align_y(Center),
                    row![
                        "Left Width",
                        space::horizontal(),
                        number_input(
                            &self.left_window_width,
                            left_window_width.as_ref().err(),
                            Message::LeftWidthChanged
                        ),
                        " ms"
                    ]
                    .align_y(Center),
                    row![
                        "Right Width",
                        space::horizontal(),
                        number_input(
                            &self.right_window_width,
                            right_window_width.as_ref().err(),
                            Message::RightWidthChanged
                        ),
                        " ms"
                    ]
                    .align_y(Center)
                ]
                .spacing(10),
                rule::horizontal(1),
                row![
                    space::horizontal(),
                    button("Close")
                        .style(button::danger)
                        .on_press(Message::Discard),
                    button("Apply")
                        .style(button::success)
                        .on_press_maybe(config.map(Message::Apply))
                ]
                .spacing(5)
            ]
            .spacing(20),
        ))
        .padding(20)
        .width(400)
        .style(container::bordered_box)
        .into()
    }
}
