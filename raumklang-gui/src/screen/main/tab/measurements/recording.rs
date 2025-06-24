mod page;

use page::{measurement, signal_setup, Component, Page};

use crate::{
    audio,
    data::recording::{self, port, volume},
    widgets::{meter, RmsPeakMeter},
};

use iced::{
    alignment::{Horizontal, Vertical},
    time,
    widget::{
        canvas, column, container, horizontal_rule, pick_list, row, rule, slider, text,
        vertical_rule,
    },
    Element, Length, Subscription, Task,
};
use tokio_stream::wrappers::ReceiverStream;

use std::{sync::Arc, time::Duration};

pub struct Recording {
    kind: Kind,
    state: State,
    page: Page,
    volume: f32,
    selected_out_port: Option<String>,
    selected_in_port: Option<String>,
    cache: canvas::Cache,
}

#[derive(Debug, Clone)]
pub enum Kind {
    Loopback,
    Measurement,
}

enum State {
    NotConnected,
    Connected {
        backend: audio::Backend,
    },
    Retrying {
        err: audio::Error,
        end: std::time::Instant,
        remaining: std::time::Duration,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    OutPortSelected(String),
    InPortSelected(String),
    RunTest(recording::port::Config),
    AudioBackend(audio::Event),
    RetryTick(time::Instant),
    VolumeChanged(f32),
    TestOk(port::Config, recording::Volume),
    RmsChanged(audio::Loudness),
    JackNotification(audio::Notification),
    Cancel,
    SignalSetup(signal_setup::Message),
    Measurement(measurement::Message),
    Back,
}

pub enum Action {
    None,
    Cancel,
    Finished(Result),
    Task(Task<Message>),
}

pub enum Result {
    Loopback(raumklang_core::Loopback),
    Measurement(raumklang_core::Measurement),
}

impl Recording {
    pub fn new(kind: Kind) -> Self {
        Self {
            kind,
            state: State::NotConnected,
            page: page::Page::default(),
            volume: 0.5,
            selected_out_port: None,
            selected_in_port: None,
            cache: canvas::Cache::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::AudioBackend(event) => match event {
                audio::Event::Ready(backend, receiver) => {
                    self.state = State::Connected { backend };

                    if let Some(receiver) = Arc::into_inner(receiver) {
                        Action::Task(
                            Task::stream(ReceiverStream::new(receiver))
                                .map(Message::JackNotification),
                        )
                    } else {
                        Action::None
                    }
                }
                audio::Event::Error { err, retry_in } => {
                    self.state = State::Retrying {
                        err,
                        end: time::Instant::now() + retry_in,
                        remaining: retry_in,
                    };
                    Action::None
                }
            },
            Message::JackNotification(notification) => {
                match notification {
                    audio::Notification::OutPortConnected(port) => {
                        self.selected_out_port = Some(port)
                    }
                    audio::Notification::OutPortDisconnected => self.selected_out_port = None,
                    audio::Notification::InPortConnected(port) => {
                        self.selected_in_port = Some(port)
                    }
                    audio::Notification::InPortDisconnected => self.selected_in_port = None,
                }

                Action::None
            }
            Message::OutPortSelected(port) => {
                let State::Connected { backend, .. } = &self.state else {
                    return Action::None;
                };

                Action::Task(Task::future(backend.clone().connect_out_port(port)).discard())
            }
            Message::InPortSelected(port) => {
                let State::Connected { backend, .. } = &self.state else {
                    return Action::None;
                };

                Action::Task(Task::future(backend.clone().connect_in_port(port)).discard())
            }
            Message::RetryTick(instant) => {
                let State::Retrying { end, remaining, .. } = &mut self.state else {
                    return Action::None;
                };

                *remaining = *end - instant;

                Action::None
            }
            Message::VolumeChanged(volume) => {
                let State::Connected { backend, .. } = &self.state else {
                    return Action::None;
                };

                self.volume = volume;

                Action::Task(Task::future(backend.clone().set_volume(volume)).discard())
            }
            Message::RmsChanged(new_loudness) => {
                let Page::LoudnessTest { loudness, .. } = &mut self.page else {
                    return Action::None;
                };

                *loudness = new_loudness;
                self.cache.clear();

                Action::None
            }
            Message::RunTest(config) => {
                let State::Connected { backend } = &mut self.state else {
                    return Action::None;
                };

                // FIXME duration not used
                let duration = Duration::from_secs(3);
                let rms_receiver = backend.run_test(duration);

                let (recv, handle) = Task::stream(ReceiverStream::new(rms_receiver))
                    .map(Message::RmsChanged)
                    .abortable();

                let handle = handle.abort_on_drop();

                self.page = Page::LoudnessTest {
                    config,
                    loudness: audio::Loudness::default(),
                    _stream_handle: handle,
                };

                Action::Task(Task::batch([
                    Task::future(backend.clone().set_volume(self.volume)).discard(),
                    recv,
                ]))
            }
            Message::TestOk(config, _volume) => {
                self.page = Page::SignalSetup {
                    config,
                    page: page::SignalSetup::new(),
                };

                Action::None
            }
            Message::SignalSetup(message) => {
                let Page::SignalSetup { page, .. } = &mut self.page else {
                    return Action::None;
                };

                match page.update(message) {
                    Some(config) => {
                        let State::Connected { backend } = &self.state else {
                            return Action::None;
                        };

                        let (page, task) = page::Measurement::new(config, backend);
                        self.page = Page::Measurement(page);

                        Action::Task(task.map(Message::Measurement))
                    }
                    None => Action::None,
                }
            }
            Message::Measurement(message) => {
                let Page::Measurement(page) = &mut self.page else {
                    return Action::None;
                };

                match page.update(message) {
                    Some(measurement) => Action::Finished(match self.kind {
                        Kind::Loopback => {
                            Result::Loopback(raumklang_core::Loopback::new(measurement))
                        }
                        Kind::Measurement => Result::Measurement(measurement),
                    }),
                    None => Action::None,
                }
            }
            Message::Cancel => Action::Cancel,
            Message::Back => {
                let page = std::mem::take(&mut self.page);

                self.page = match page {
                    Page::PortSetup => page,
                    Page::LoudnessTest { .. } => Page::PortSetup,
                    Page::SignalSetup { .. } => Page::PortSetup,
                    Page::Measurement(_measurement) => Page::PortSetup,
                };

                Action::None
            }
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let page = match &self.state {
            State::NotConnected => {
                page::Component::new("Jack").content(text("Jack is not connected."))
            }
            State::Connected { backend } => match &self.page {
                Page::PortSetup => self.port_setup(backend),
                Page::LoudnessTest {
                    config, loudness, ..
                } => self
                    .loudness_test(config, loudness)
                    .back_button("Back", Message::Back),
                Page::SignalSetup { config, page } => page
                    .view(config)
                    .map(Message::SignalSetup)
                    .back_button("Back", Message::Back),
                Page::Measurement(page) => page.view().map(Message::Measurement),
            },
            State::Retrying { err, remaining, .. } => page::Component::new("Jack error").content(
                container(
                    column![
                        text("Connection to Jack audio server failed:")
                            .size(18)
                            .style(text::danger),
                        text!("{err}").style(text::danger),
                        column![
                            text("Retrying in").size(14),
                            text!("{} s", remaining.as_secs()).size(18)
                        ]
                        .padding(8)
                        .align_x(Horizontal::Center),
                    ]
                    .align_x(Horizontal::Center)
                    .spacing(16),
                )
                .center_x(Length::Fill),
            ),
        };

        let page = page.cancel_button("Cancel", Message::Cancel);

        container(page).width(Length::Fill).into()
    }

    fn port_setup<'a>(&'a self, backend: &'a audio::Backend) -> page::Component<'a, Message> {
        Component::new("Port Setup")
            .content(
                row![
                    column![
                        text("Out port"),
                        pick_list(
                            backend.out_ports.as_slice(),
                            self.selected_out_port.as_ref(),
                            Message::OutPortSelected
                        )
                    ]
                    .spacing(6),
                    column![
                        text("In port"),
                        pick_list(
                            backend.in_ports.as_slice(),
                            self.selected_in_port.as_ref(),
                            Message::InPortSelected
                        )
                    ]
                    .spacing(6),
                ]
                .spacing(12),
            )
            .next_button(
                "Start test",
                recording::port::Config::new(
                    self.selected_out_port.clone(),
                    self.selected_in_port.clone(),
                )
                .map(Message::RunTest),
            )
    }

    fn loudness_test(
        &self,
        config: &port::Config,
        loudness: &audio::Loudness,
    ) -> page::Component<'_, Message> {
        fn loudness_text<'a>(label: &'a str, value: f32) -> Element<'a, Message> {
            column![
                text(label).size(12).align_y(Vertical::Bottom),
                horizontal_rule(1),
                text!("{:.1}", value).size(24),
            ]
            .spacing(3)
            .width(Length::Shrink)
            .align_x(Horizontal::Center)
            .into()
        }

        let volume = recording::Volume::new(self.volume, loudness);
        Component::new("Loudness Test ...")
            .content(
                row![
                    container(
                        canvas(
                            RmsPeakMeter::new(loudness.rms, loudness.peak, &self.cache).state(
                                match volume {
                                    Ok(_) => meter::State::Normal,
                                    Err(volume::ValidationError::ToLow(_)) => meter::State::Warning,
                                    Err(volume::ValidationError::ToHigh(_)) => meter::State::Danger,
                                }
                            )
                        )
                        .width(60)
                        .height(200)
                    )
                    .padding(10),
                    column![
                        container(
                            row![
                                loudness_text("RMS", loudness.rms),
                                vertical_rule(3).style(|theme| {
                                    let mut style = rule::default(theme);
                                    style.width = 3;
                                    style
                                }),
                                loudness_text("Peak", loudness.peak),
                            ]
                            .align_y(Vertical::Bottom)
                            .height(Length::Shrink)
                            .spacing(10)
                        )
                        .center_x(Length::Fill),
                        slider(0.0..=1.0, self.volume, Message::VolumeChanged).step(0.01),
                    ]
                    .spacing(10)
                ]
                .align_y(Vertical::Center),
            )
            .next_button(
                "Next",
                volume
                    .ok()
                    .map(|volume| Message::TestOk(config.clone(), volume)),
            )
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let audio_backend = Subscription::run(audio::run).map(Message::AudioBackend);

        let mut subscriptions = vec![audio_backend];

        if let State::Retrying { .. } = &self.state {
            subscriptions.push(time::every(Duration::from_millis(500)).map(Message::RetryTick));
        }

        Subscription::batch(subscriptions)
    }
}

impl Default for Recording {
    fn default() -> Self {
        Self::new(Kind::Measurement)
    }
}
