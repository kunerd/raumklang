use std::time::Duration;

use crate::{audio, data, widgets::colored_circle};
use iced::{
    alignment::Vertical,
    time,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, pick_list, row, slider, text,
        text_input, Button,
    },
    Alignment, Color, Element, Length, Subscription, Task,
};
use pliced::chart::{line_series, Chart};
use tokio_stream::wrappers::ReceiverStream;

pub struct Recording {
    state: State,
    volume: f32,
    selected_out_port: Option<String>,
    selected_in_port: Option<String>,
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
}

pub enum Action {
    None,
    Back,
    Task(Task<Message>),
}

impl Recording {
    pub fn new() -> Self {
        Self {
            state: State::NotConnected,
            volume: 0.5,
            selected_out_port: None,
            selected_in_port: None,
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

                *measurement = MeasurementState::Testing {
                    loudness: audio::Loudness::default(),
                };

                Action::Task(Task::batch([
                    Task::future(backend.clone().set_volume(self.volume)).discard(),
                    Task::stream(ReceiverStream::new(rms_receiver)).map(Message::RmsChanged),
                ]))
            }
            Message::AudioBackend(event) => match event {
                audio::Event::Ready(backend) => {
                    self.state = State::Connected {
                        backend,
                        measurement: MeasurementState::Init,
                    };
                    Action::None
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
                audio::Event::Notification(notification) => {
                    let State::Connected { .. } = self.state else {
                        return Action::None;
                    };

                    dbg!(&notification);
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
            },
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
                let State::Connected {
                    backend,
                    measurement,
                } = &mut self.state
                else {
                    return Action::None;
                };

                backend.stop_test();

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

                Action::None
            }
            Message::TestOk => {
                let State::Connected {
                    backend,
                    measurement: measurement @ MeasurementState::Testing { .. },
                } = &mut self.state
                else {
                    return Action::None;
                };

                backend.stop_test();

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
                    let (loudness_receiver, data_receiver) =
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

                    Action::Task(Task::batch(vec![
                        Task::stream(ReceiverStream::new(loudness_receiver))
                            .map(Message::RmsChanged),
                        Task::stream(ReceiverStream::new(data_receiver))
                            .map(Message::RecordingChunk),
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
                    let header = |subsection| {
                        column![
                            row![
                                text!("Recording - {subsection}").size(20),
                                horizontal_space(),
                                text!("Sample rate: {}", backend.sample_rate).size(14)
                            ]
                            .align_y(Vertical::Bottom),
                            horizontal_rule(1),
                        ]
                        .spacing(4)
                    };

                    match measurement {
                        MeasurementState::Init => container(
                            column![
                                header("Setup"),
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
                                row![
                                    button("Cancel").on_press(Message::Back),
                                    button("Start test"),
                                    horizontal_space(),
                                ]
                                .spacing(12)
                            ]
                            .spacing(18),
                        )
                        .style(container::bordered_box)
                        .padding(18)
                        .into(),
                        MeasurementState::ReadyForTest => container(
                            column![
                                header("Ready for Test"),
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
                                row![
                                    button("Cancel").on_press(Message::Back),
                                    button("Start test").on_press(Message::RunTest),
                                    horizontal_space(),
                                ]
                                .spacing(12)
                            ]
                            .spacing(18),
                        )
                        .style(container::bordered_box)
                        .padding(18)
                        .into(),
                        MeasurementState::Testing { loudness, .. } => container(column![
                            header("Loudness Test..."),
                            text!("RMS: {}, Peak: {}", loudness.rms, loudness.peak),
                            slider(0.0..=1.0, self.volume, Message::VolumeChanged).step(0.01),
                            row![
                                button("Stop").on_press(Message::StopTesting),
                                // TODO: enable button when loudness levels are ok
                                button("Ok").on_press(Message::TestOk)
                            ]
                        ])
                        .style(container::bordered_box)
                        .padding(18)
                        .into(),
                        MeasurementState::PreparingMeasurement {
                            duration,
                            start_frequency,
                            end_frequency,
                        } => container(
                            column![
                                header("Prepare Measurement"),
                                row![
                                    column![
                                        text("Out port"),
                                        container(text!(
                                            "{}",
                                            self.selected_in_port.as_ref().unwrap()
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
                                row![
                                    button("Cancel").on_press(Message::Back),
                                    button("Start Measurement").on_press(Message::RunMeasurement),
                                    horizontal_space(),
                                ]
                                .spacing(12)
                            ]
                            .spacing(18),
                        )
                        .style(container::bordered_box)
                        .padding(18)
                        .into(),
                        MeasurementState::MeasurementRunning {
                            loudness,
                            data,
                            finished_len,
                        } => container(
                            column![
                                header("Measurement Running ..."),
                                row![
                                    column![
                                        column![
                                            text("Out port"),
                                            container(text!(
                                                "{}",
                                                self.selected_out_port.as_ref().unwrap()
                                            ))
                                            .padding(4)
                                            .style(container::rounded_box)
                                        ]
                                        .spacing(6),
                                        column![
                                            text("In port"),
                                            container(text!(
                                                "{}",
                                                self.selected_in_port.as_ref().unwrap()
                                            ))
                                            .padding(4)
                                            .style(container::rounded_box)
                                        ]
                                        .spacing(6),
                                        column![
                                            text!("Rms: {}", loudness.rms),
                                            text!("Peak: {}", loudness.peak),
                                            text!("Data len: {}", data.len())
                                        ]
                                        .spacing(6),
                                    ]
                                    .spacing(12)
                                    .padding(6),
                                    Chart::<_, (), _>::new()
                                        .x_range(0.0..=*finished_len as f32)
                                        .y_range(-1.0..=1.0)
                                        .push_series(
                                            line_series(
                                                data.iter()
                                                    .enumerate()
                                                    .map(|(i, s)| (i as f32, *s))
                                            )
                                            .color(iced::Color::from_rgb8(200, 200, 34))
                                        )
                                ]
                                .spacing(12),
                                row![
                                    button("Stop").on_press(Message::StopTesting),
                                    horizontal_space(),
                                ]
                                .spacing(12)
                            ]
                            .spacing(18),
                        )
                        .style(container::bordered_box)
                        .padding(18)
                        .into(),
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

pub fn recording_button<'a, Message: 'a>(msg: Message) -> Button<'a, Message> {
    button(colored_circle(8.0, Color::from_rgb8(200, 56, 42))).on_press(msg)
}
