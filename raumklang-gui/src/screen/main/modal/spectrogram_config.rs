use std::time::Duration;

use crate::{data::spectrogram, icon, widget::number_input};

use iced::{
    Alignment::Center,
    Element,
    widget::{button, column, container, row, rule, scrollable, space, text, tooltip},
};

#[derive(Debug, Clone)]
pub enum Message {
    Close,
    ResetToDefault,
    ResetToPrevious,
    WindowWidthChanged(String),
    SpanBeforePeakChanged(String),
    SpanAfterPeakChanged(String),
    Apply(spectrogram::Config),
}

pub enum Action {
    None,
    Close,
    ConfigChanged(spectrogram::Config),
}

#[derive(Debug, Clone)]
pub struct SpectrogramConfig {
    window_width: String,
    span_before_peak: String,
    span_after_peak: String,
    prev_config: spectrogram::Config,
}

impl SpectrogramConfig {
    pub fn new(config: spectrogram::Config) -> Self {
        Self {
            window_width: config.window_width.as_millis().to_string(),
            span_before_peak: config.span_before_peak.as_millis().to_string(),
            span_after_peak: config.span_after_peak.as_millis().to_string(),
            prev_config: config,
        }
    }

    #[must_use]
    pub fn update(&mut self, message: Message) -> Action {
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
            Message::ResetToDefault => {
                self.reset_to_default();
                Action::None
            }
            Message::ResetToPrevious => {
                self.reset_to_config(self.prev_config.clone());
                Action::None
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let window_width = self.window_width.parse().map(Duration::from_millis);
        let span_before_peak = self.span_before_peak.parse().map(Duration::from_millis);
        let span_after_peak = self.span_after_peak.parse().map(Duration::from_millis);

        let config = if let (Ok(window_width), Ok(span_before_peak), Ok(span_after_peak)) = (
            window_width.as_ref(),
            span_before_peak.as_ref(),
            span_after_peak.as_ref(),
        ) {
            let new_config = spectrogram::Config {
                window_width: *window_width,
                span_before_peak: *span_before_peak,
                span_after_peak: *span_after_peak,
            };

            if new_config != self.prev_config {
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
                    tooltip(
                        button(icon::reset().center())
                            .on_press(Message::ResetToDefault)
                            .style(button::secondary),
                        "Reset to defaults.",
                        tooltip::Position::default()
                    )
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
                    tooltip(
                        button(icon::reset().center())
                            .on_press_maybe(config.is_some().then_some(Message::ResetToPrevious))
                            .style(button::secondary),
                        "Reset to defaults.",
                        tooltip::Position::default()
                    ),
                    button("Close")
                        .style(button::secondary)
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

    fn reset_to_default(&mut self) {
        self.reset_to_config(spectrogram::Config::default());
    }

    fn reset_to_config(&mut self, config: spectrogram::Config) {
        self.window_width = config.window_width.as_millis().to_string();
        self.span_before_peak = config.span_before_peak.as_millis().to_string();
        self.span_after_peak = config.span_after_peak.as_millis().to_string();
    }
}
