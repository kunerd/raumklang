use std::{sync::Arc, time::Duration};

use crate::{
    audio, data,
    widgets::{colored_circle, RmsPeakMeter},
};
use iced::{
    alignment::{Horizontal, Vertical},
    task, time,
    widget::{
        button, canvas, column, container, horizontal_rule, horizontal_space, pick_list, row, rule,
        slider, text, text_input, vertical_rule, Button,
    },
    Alignment, Color, Element, Length, Subscription, Task,
};
use prism::{line_series, Chart};
use tokio_stream::wrappers::ReceiverStream;

pub struct Recording {
    kind: Kind,
    state: State,
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
        measurement: MeasurementState,
    },
    Retrying {
        err: Option<audio::Error>,
        end: std::time::Instant,
        remaining: std::time::Duration,
    },
    Error(audio::Error),
}

enum MeasurementState {
    Init,
    ReadyForTest,
    Testing {
        loudness: audio::Loudness,
        _stream_handle: task::Handle,
    },
    PreparingMeasurement {
        duration: Duration,
        start_frequency: u16,
        end_frequency: u16,
    },
    MeasurementRunning {
        finished_len: usize,
        loudness: audio::Loudness,
        data: Vec<f32>,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    OutPortSelected(String),
    InPortSelected(String),
    RunTest,
    AudioBackend(audio::Event),
    RetryTick(time::Instant),
    VolumeChanged(f32),
    StopTesting,
    TestOk,
    RmsChanged(audio::Loudness),
    RunMeasurement,
    RecordingChunk(Box<[f32]>),
    JackNotification(audio::Notification),
    RecordingFinished,
}

pub enum Action {
    None,
    Back,
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
            volume: 0.5,
            selected_out_port: None,
            selected_in_port: None,
            cache: canvas::Cache::new(),
        }
    }

    pub fn update(&mut self, message: Message, sample_rate: data::SampleRate) -> Action {
        match message {
            Message::Back => Action::Back,
            Message::RunTest => {
                let State::Connected {
                    backend,
                    measurement: measurement @ MeasurementState::ReadyForTest,
                } = &mut self.state
                else {
                    return Action::None;
                };

                let duration = Duration::from_secs(3);
                let rms_receiver = backend.run_test(duration);

                let (recv, handle) = Task::stream(ReceiverStream::new(rms_receiver))
                    .map(Message::RmsChanged)
                    .abortable();

                let handle = handle.abort_on_drop();

                *measurement = MeasurementState::Testing {
                    loudness: audio::Loudness::default(),
                    _stream_handle: handle,
                };

                Action::Task(Task::batch([
                    Task::future(backend.clone().set_volume(self.volume)).discard(),
                    recv,
                ]))
            }
            Message::AudioBackend(event) => match event {
                audio::Event::Ready(backend, receiver) => {
                    self.state = State::Connected {
                        backend,
                        measurement: MeasurementState::Init,
                    };

                    if let Some(receiver) = Arc::into_inner(receiver) {
                        Action::Task(
                            Task::stream(ReceiverStream::new(receiver))
                                .map(Message::JackNotification),
                        )
                    } else {
                        Action::None
                    }
                }
                audio::Event::Error(err) => {
                    println!("{err}");
                    self.state = State::Error(err);
                    Action::None
                }
                audio::Event::RetryIn(timeout) => {
                    let err = if let State::Error(err) = &self.state {
                        Some(err.clone())
                    } else {
                        None
                    };

                    self.state = State::Retrying {
                        err,
                        end: time::Instant::now() + timeout,
                        remaining: timeout,
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

                self.check_port_state();

                Action::None
            }
            Message::OutPortSelected(dest_port) => {
                let State::Connected { backend, .. } = &self.state else {
                    return Action::None;
                };

                Action::Task(Task::future(backend.clone().connect_out_port(dest_port)).discard())
            }
            Message::InPortSelected(dest_port) => {
                let State::Connected { backend, .. } = &self.state else {
                    return Action::None;
                };

                Action::Task(Task::future(backend.clone().connect_in_port(dest_port)).discard())
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
            Message::StopTesting => {
                let State::Connected { measurement, .. } = &mut self.state else {
                    return Action::None;
                };

                *measurement = match measurement {
                    MeasurementState::Init => MeasurementState::Init,
                    MeasurementState::ReadyForTest => MeasurementState::ReadyForTest,
                    MeasurementState::Testing { .. } => MeasurementState::ReadyForTest,
                    MeasurementState::PreparingMeasurement { .. } => MeasurementState::ReadyForTest,
                    MeasurementState::MeasurementRunning { .. } => MeasurementState::ReadyForTest,
                };

                Action::None
            }
            Message::RmsChanged(new_loudness) => {
                let State::Connected {
                    measurement:
                        MeasurementState::Testing { loudness, .. }
                        | MeasurementState::MeasurementRunning { loudness, .. },
                    ..
                } = &mut self.state
                else {
                    return Action::None;
                };

                *loudness = new_loudness;
                self.cache.clear();

                Action::None
            }
            Message::TestOk => {
                let State::Connected {
                    measurement: measurement @ MeasurementState::Testing { .. },
                    ..
                } = &mut self.state
                else {
                    return Action::None;
                };

                let nquist = Into::<u32>::into(sample_rate) as u16 / 2 - 1;
                *measurement = MeasurementState::PreparingMeasurement {
                    duration: Duration::from_secs(3),
                    start_frequency: 20,
                    end_frequency: nquist,
                };

                Action::None
            }
            Message::RunMeasurement => {
                let State::Connected {
                    backend,
                    measurement,
                } = &mut self.state
                else {
                    return Action::None;
                };

                let state = std::mem::replace(measurement, MeasurementState::Init);
                if let MeasurementState::PreparingMeasurement {
                    duration,
                    start_frequency,
                    end_frequency,
                } = state
                {
                    let (loudness_receiver, mut data_receiver) =
                        backend.run_measurement(start_frequency, end_frequency, duration);

                    *measurement = MeasurementState::MeasurementRunning {
                        finished_len: data::Samples::from_duration(
                            duration,
                            data::SampleRate::new(44_100),
                        )
                        .into(),
                        loudness: audio::Loudness::default(),
                        data: vec![],
                    };

                    let measurement_sipper = iced::task::sipper(async move |mut progress| {
                        while let Some(data) = data_receiver.recv().await {
                            progress.send(data).await;
                        }
                    });

                    Action::Task(Task::batch(vec![
                        Task::stream(ReceiverStream::new(loudness_receiver))
                            .map(Message::RmsChanged),
                        Task::sip(measurement_sipper, Message::RecordingChunk, |_| {
                            Message::RecordingFinished
                        }),
                    ]))
                } else {
                    *measurement = state;
                    Action::None
                }
            }
            Message::RecordingChunk(chunk) => {
                let State::Connected {
                    backend: _,
                    measurement: MeasurementState::MeasurementRunning { data, .. },
                } = &mut self.state
                else {
                    return Action::None;
                };

                data.extend_from_slice(&chunk);

                Action::None
            }
            Message::RecordingFinished => {
                let State::Connected {
                    backend,
                    measurement: MeasurementState::MeasurementRunning { data, .. },
                } = &mut self.state
                else {
                    return Action::None;
                };

                let sample_rate = backend.sample_rate.into();
                let data = std::mem::replace(data, Vec::new());

                let result = match self.kind {
                    Kind::Loopback => {
                        Result::Loopback(raumklang_core::Loopback::new(sample_rate, data))
                    }
                    Kind::Measurement => {
                        Result::Measurement(raumklang_core::Measurement::new(sample_rate, data))
                    }
                };

                Action::Finished(result)
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<_> = {
            match &self.state {
                State::NotConnected => container(text("Jack is not connected."))
                    .center(Length::Fill)
                    .into(),
                State::Connected {
                    backend,
                    measurement,
                } => {
                    let setup_page = |sample_rate, start_test_msg| -> Element<'_, Message> {
                        Page::new("Setup")
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
                            .push_button(button("Cancel").on_press(Message::Back))
                            .push_button(button("Start test").on_press_maybe(start_test_msg))
                            .view(sample_rate)
                    };

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

                    let sample_rate = &backend.sample_rate;
                    match measurement {
                        MeasurementState::Init => setup_page(sample_rate, None),
                        MeasurementState::ReadyForTest => {
                            setup_page(sample_rate, Some(Message::RunTest))
                        }
                        MeasurementState::Testing { loudness, .. } => {
                            Page::new("Loudness Test ...")
                                .content(
                                    row![
                                        container(
                                            canvas(RmsPeakMeter::new(
                                                loudness.rms,
                                                loudness.peak,
                                                &self.cache
                                            ))
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
                                            slider(0.0..=1.0, self.volume, Message::VolumeChanged)
                                                .step(0.01),
                                        ]
                                        .spacing(10)
                                    ]
                                    .align_y(Vertical::Center),
                                )
                                .push_button(button("Stop").on_press(Message::StopTesting))
                                .push_button(button("Ok").on_press(Message::TestOk))
                                .view(sample_rate)
                        }
                        MeasurementState::PreparingMeasurement {
                            duration,
                            start_frequency,
                            end_frequency,
                        } => Page::new("Setup Measurement")
                            .content(
                                column![
                                    row![
                                        column![
                                            text("Out port"),
                                            container(text!(
                                                "{}",
                                                self.selected_out_port.as_ref().unwrap()
                                            ))
                                            .padding(3)
                                            .style(container::rounded_box)
                                        ]
                                        .spacing(6),
                                        column![
                                            text("In port"),
                                            container(text!(
                                                "{}",
                                                self.selected_in_port.as_ref().unwrap()
                                            ))
                                            .padding(3)
                                        ]
                                        .spacing(6),
                                    ]
                                    .spacing(12),
                                    row![
                                        column![
                                            text("Frequency"),
                                            row![
                                                text("From"),
                                                text_input("From", &format!("{}", start_frequency)),
                                                text("To"),
                                                text_input("To", &format!("{}", end_frequency))
                                            ]
                                            .spacing(8)
                                            .align_y(Alignment::Center),
                                        ]
                                        .spacing(6),
                                        row![
                                            column![
                                                text("Duration"),
                                                text_input(
                                                    "Duration",
                                                    &format!("{}", duration.as_secs())
                                                )
                                            ]
                                            .spacing(6),
                                            horizontal_space()
                                        ]
                                    ]
                                    .spacing(8)
                                    .align_y(Alignment::Center),
                                ]
                                .spacing(12),
                            )
                            .push_button(button("Cancel").on_press(Message::Back))
                            .push_button(
                                button("Start Measurement").on_press(Message::RunMeasurement),
                            )
                            .view(sample_rate),
                        MeasurementState::MeasurementRunning {
                            loudness,
                            data,
                            finished_len,
                        } => Page::new("Measurement Running ...")
                            .content(
                                row![
                                    container(
                                        canvas(RmsPeakMeter::new(
                                            loudness.rms,
                                            loudness.peak,
                                            &self.cache
                                        ))
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
                                        Chart::<_, (), _>::new()
                                            .x_range(0.0..=*finished_len as f32)
                                            .y_range(-0.5..=0.5)
                                            .push_series(
                                                line_series(
                                                    data.iter()
                                                        .enumerate()
                                                        .map(|(i, s)| (i as f32, *s))
                                                )
                                                .color(
                                                    iced::Color::from_rgb8(50, 175, 50)
                                                        .scale_alpha(0.6)
                                                )
                                            )
                                    ]
                                    .spacing(12)
                                    .padding(10)
                                ]
                                .spacing(12)
                                .align_y(Vertical::Center),
                            )
                            .push_button(button("Stop").on_press(Message::StopTesting))
                            .view(sample_rate),
                    }
                }
                State::Retrying { err, remaining, .. } => container(
                    column![text("Something went wrong."),]
                        .push_maybe(err.as_ref().map(|err| text!("{err}")))
                        .push(text!("Retrying in: {}s", remaining.as_secs()))
                        .spacing(8),
                )
                .center_x(Length::Fill)
                .into(),
                State::Error(error) => container(column![
                    text("Something went wrong."),
                    text!("Error: {error}")
                ])
                .center(Length::Fill)
                .into(),
            }
        };

        container(content).width(Length::Fill).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let audio_backend = Subscription::run(audio::run).map(Message::AudioBackend);

        let mut subscriptions = vec![audio_backend];

        if let State::Retrying { .. } = &self.state {
            subscriptions.push(time::every(Duration::from_millis(500)).map(Message::RetryTick));
        }

        Subscription::batch(subscriptions)
    }

    fn check_port_state(&mut self) {
        let State::Connected { measurement, .. } = &mut self.state else {
            return;
        };

        match measurement {
            MeasurementState::Init => {
                if self.selected_in_port.is_some() && self.selected_out_port.is_some() {
                    *measurement = MeasurementState::ReadyForTest;
                }
            }
            MeasurementState::ReadyForTest => {
                if self.selected_in_port.is_none() || self.selected_out_port.is_none() {
                    *measurement = MeasurementState::Init;
                }
            }
            MeasurementState::Testing { .. } => {}
            MeasurementState::PreparingMeasurement { .. } => {}
            MeasurementState::MeasurementRunning { .. } => {}
        }
    }
}

impl Default for Recording {
    fn default() -> Self {
        Self::new(Kind::Measurement)
    }
}

pub fn recording_button<'a, Message: 'a>(msg: Message) -> Button<'a, Message> {
    button(colored_circle(8.0, Color::from_rgb8(200, 56, 42))).on_press(msg)
}

struct Page<'a, Message> {
    title: &'a str,
    content: Option<Element<'a, Message>>,
    buttons: Vec<Element<'a, Message>>,
}

impl<'a, Message> Page<'a, Message>
where
    Message: 'a,
{
    fn new(title: &'a str) -> Self {
        Self {
            title,
            content: None,
            buttons: Vec::new(),
        }
    }

    fn content(mut self, content: impl Into<Element<'a, Message>>) -> Self {
        self.content = Some(content.into());
        self
    }

    fn push_button(mut self, button: impl Into<Element<'a, Message>>) -> Self {
        self.buttons.push(button.into());
        self
    }

    fn view(self, sample_rate: &'a data::SampleRate) -> Element<'a, Message> {
        let header = |subsection| {
            column![
                row![
                    text!("Recording - {subsection}").size(20),
                    horizontal_space(),
                    text!("Sample rate: {}", sample_rate).size(14)
                ]
                .align_y(Vertical::Bottom),
                horizontal_rule(1),
            ]
            .spacing(4)
        };

        container(
            column![header(&self.title)]
                .push_maybe(self.content)
                .push(container(row(self.buttons).spacing(12)).align_right(Length::Fill))
                .spacing(18),
        )
        .style(container::bordered_box)
        .padding(18)
        .into()
    }
}
