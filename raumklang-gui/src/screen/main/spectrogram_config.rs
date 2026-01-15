use std::time::Duration;

use crate::{data::spectrogram, icon, widget::number_input};

use iced::{
    widget::{button, column, container, row, rule, scrollable, space, text},
    Alignment::Center,
    Element,
};

#[derive(Debug, Clone)]
pub(crate) enum Message {
    Close,
    WindowWidthChanged(String),
    SpanBeforePeakChanged(String),
    SpanAfterPeakChanged(String),
    Apply(spectrogram::Preferences),
}

pub(crate) enum Action {
    None,
    Close,
    ConfigChanged(spectrogram::Preferences),
}

#[derive(Debug, Clone)]
pub(crate) struct SpectrogramConfig {
    window_width: String,
    span_before_peak: String,
    span_after_peak: String,
    original_config: spectrogram::Preferences,
}

impl SpectrogramConfig {
    pub(crate) fn new(config: spectrogram::Preferences) -> Self {
        Self {
            window_width: config.window_width.as_millis().to_string(),
            span_before_peak: config.span_before_peak.as_millis().to_string(),
            span_after_peak: config.span_after_peak.as_millis().to_string(),
            original_config: config,
        }
    }

    #[must_use]
    pub(crate) fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Close => Action::Close,
            Message::WindowWidthChanged(width) => {
                self.window_width = width;

                Action::None
            }
            Message::SpanBeforePeakChanged(span) => {
                self.span_before_peak = span;

                Action::None
            }
            Message::SpanAfterPeakChanged(span) => {
                self.span_after_peak = span;

                Action::None
            }
            Message::Apply(preferences) => Action::ConfigChanged(preferences),
        }
    }

    pub(crate) fn view(&self) -> Element<'_, Message> {
        let window_width = self.window_width.parse().map(Duration::from_millis);
        let span_before_peak = self.span_before_peak.parse().map(Duration::from_millis);
        let span_after_peak = self.span_after_peak.parse().map(Duration::from_millis);

        let config = if let (Ok(window_width), Ok(span_before_peak), Ok(span_after_peak)) = (
            window_width.as_ref(),
            span_before_peak.as_ref(),
            span_after_peak.as_ref(),
        ) {
            let new_config = spectrogram::Preferences {
                window_width: *window_width,
                span_before_peak: *span_before_peak,
                span_after_peak: *span_after_peak,
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
                    text("Spectrogram Config").size(18),
                    space::horizontal(),
                    button(icon::reset().center()).style(button::secondary)
                ],
                rule::horizontal(1),
                column![
                    row![
                        "Window width",
                        space::horizontal(),
                        number_input(
                            &self.window_width,
                            window_width.as_ref().err(),
                            Message::WindowWidthChanged
                        ),
                        " ms"
                    ]
                    .align_y(Center),
                    row![
                        "Span before peak",
                        space::horizontal(),
                        number_input(
                            &self.span_before_peak,
                            span_before_peak.as_ref().err(),
                            Message::SpanBeforePeakChanged
                        ),
                        " ms"
                    ]
                    .align_y(Center),
                    row![
                        "Span after peak",
                        space::horizontal(),
                        number_input(
                            &self.span_after_peak,
                            span_after_peak.as_ref().err(),
                            Message::SpanAfterPeakChanged
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
                        .on_press(Message::Close),
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
