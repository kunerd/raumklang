mod page;

use page::{signal_setup, Component, Page};

use crate::{
    audio,
    data::{
        self,
        measurement::{self, config},
        recording::{self, port},
    },
    log,
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

// enum MeasurementState {
//     Init,
//     ReadyForTest,
//     Testing {
//         loudness: audio::Loudness,
//         _stream_handle: task::Handle,
//     },
//     PreparingMeasurement(ConfigFields),
//     MeasurementRunning {
//         finished_len: usize,
//         loudness: audio::Loudness,
//         data: Vec<f32>,
//     },
// }

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    OutPortSelected(String),
    InPortSelected(String),
    RunTest(recording::port::Config),
    AudioBackend(audio::Event),
    RetryTick(time::Instant),
    VolumeChanged(f32),
    StopTesting,
    TestOk(port::Config, recording::Volume),
    RmsChanged(audio::Loudness),
    RecordingChunk(Box<[f32]>),
    JackNotification(audio::Notification),
    RecordingFinished,
    Cancel,
    SignalSetup(signal_setup::Message),
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

#[derive(Debug, Clone)]
pub enum Field {
    StartFrequency(String),
    EndFrequency(String),
    Duration(String),
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
            Message::Back => Action::None,
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
            Message::StopTesting => {
                // let State::Connected { measurement, .. } = &mut self.state else {
                //     return Action::None;
                // };

                // *measurement = match measurement {
                //     MeasurementState::Init => MeasurementState::Init,
                //     MeasurementState::ReadyForTest => MeasurementState::ReadyForTest,
                //     MeasurementState::Testing { .. } => MeasurementState::ReadyForTest,
                //     MeasurementState::PreparingMeasurement { .. } => MeasurementState::ReadyForTest,
                //     MeasurementState::MeasurementRunning { .. } => MeasurementState::ReadyForTest,
                // };

                Action::None
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
                        let State::Connected { backend } = &mut self.state else {
                            return Action::None;
                        };

                        let finished_len =
                            data::Samples::from_duration(config.duration(), backend.sample_rate)
                                .into();

                        self.page = Page::MeasurementRunning {
                            finished_len,
                            loudness: audio::Loudness::default(),
                            data: vec![],
                        };

                        let (loudness_receiver, mut data_receiver) =
                            backend.run_measurement(config);

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
                    }
                    None => Action::None,
                }
            }
            Message::RecordingChunk(chunk) => {
                let Page::MeasurementRunning { data, .. } = &mut self.page else {
                    return Action::None;
                };

                data.extend_from_slice(&chunk);

                Action::None
            }
            Message::RecordingFinished => {
                let State::Connected { backend } = &mut self.state else {
                    return Action::None;
                };

                let Page::MeasurementRunning { data, .. } = &mut self.page else {
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
            Message::Cancel => Action::Cancel,
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let page = match &self.state {
            State::NotConnected => {
                page::Component::new("Jack").content(text("Jack is not connected."))
            }
            State::Connected { backend } => {
                match &self.page {
                    Page::PortSetup => self.port_setup(backend),
                    Page::LoudnessTest {
                        config, loudness, ..
                    } => self.loudness_test(config, loudness),
                    Page::SignalSetup { config, page } => {
                        page.view(config).map(Message::SignalSetup)
                    }
                    Page::MeasurementRunning {
                        finished_len,
                        loudness,
                        data,
                    } => {
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

                        Component::new("Measurement Running ...").content(
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
                        // .push_button(button("Stop").on_press(Message::StopTesting))
                    }
                }
                // page::Component::new(self.page.to_string())
                //     .content(match self.page {
                //         page::Page::PortSetup => self.port_setup(backend),
                //         page::Page::LoudnessTest { loudness, .. } => self.loudness_test(loudness),
                //         page::Page::MeasurementSetup(ref fields) => {
                //             let range = config::FrequencyRange::from_strings(
                //                 &fields.start_frequency,
                //                 &fields.end_frequency,
                //             );
                //             let range_err = range.is_err();
                //             let duration = fields.duration.parse().map(Duration::from_secs_f32);
                //             let duration_err = duration.is_err();
                //             column![
                //                 row![
                //                     column![
                //                         text("Out port"),
                //                         container(text!(
                //                             "{}",
                //                             self.selected_out_port.as_ref().unwrap()
                //                         ))
                //                         .padding(3)
                //                         .style(container::rounded_box)
                //                     ]
                //                     .spacing(6),
                //                     column![
                //                         text("In port"),
                //                         container(text!(
                //                             "{}",
                //                             self.selected_in_port.as_ref().unwrap()
                //                         ))
                //                         .padding(3)
                //                     ]
                //                     .spacing(6),
                //                 ]
                //                 .spacing(12),
                //                 row![
                //                     {
                //                         let color = move |theme: &iced::Theme| {
                //                             if range_err {
                //                                 theme.extended_palette().danger.weak.color
                //                             } else {
                //                                 theme.extended_palette().secondary.strong.color
                //                             }
                //                         };

                //                         container(
                //                             column![text("Frequency"), horizontal_rule(1),]
                //                                 .push_maybe(
                //                                     range.as_ref().err().map(|err| text!("{err}")),
                //                                 )
                //                                 .push(
                //                                     row![
                //                                         text("From"),
                //                                         text_input("From", &fields.start_frequency)
                //                                             .on_input(|s| {
                //                                                 Message::ConfigFieldChanged(
                //                                                     Field::StartFrequency(s),
                //                                                 )
                //                                             })
                //                                             .style(move |theme, status| {
                //                                                 let mut style = text_input::default(
                //                                                     theme, status,
                //                                                 );
                //                                                 style.border = style
                //                                                     .border
                //                                                     .color(color(theme));
                //                                                 style
                //                                             }),
                //                                         text("To"),
                //                                         text_input("To", &fields.end_frequency)
                //                                             .on_input(|s| {
                //                                                 Message::ConfigFieldChanged(
                //                                                     Field::EndFrequency(s),
                //                                                 )
                //                                             }),
                //                                     ]
                //                                     .spacing(8)
                //                                     .align_y(Alignment::Center),
                //                                 )
                //                                 .spacing(6),
                //                         )
                //                         .style(move |theme| {
                //                             let style = container::rounded_box(theme);
                //                             if range_err {
                //                                 style.color(color(theme))
                //                             } else {
                //                                 style
                //                             }
                //                         })
                //                         .padding(8)
                //                     },
                //                     container(row![
                //                         column![
                //                             text("Duration"),
                //                             horizontal_rule(1),
                //                             text_input("Duration", &fields.duration)
                //                                 .on_input(|s| Message::ConfigFieldChanged(
                //                                     Field::Duration(s)
                //                                 ))
                //                                 .style(move |theme: &iced::Theme, status| {
                //                                     if duration_err {
                //                                         text_input::Style {
                //                                             border: iced::Border {
                //                                                 color: theme
                //                                                     .extended_palette()
                //                                                     .danger
                //                                                     .base
                //                                                     .color,
                //                                                 width: 1.0,
                //                                                 ..Default::default()
                //                                             },
                //                                             ..text_input::default(theme, status)
                //                                         }
                //                                     } else {
                //                                         text_input::default(theme, status)
                //                                     }
                //                                 }),
                //                         ]
                //                         .spacing(8),
                //                         horizontal_space()
                //                     ])
                //                     .style(move |theme| {
                //                         let style = container::rounded_box(theme);
                //                         if duration_err {
                //                             style
                //                                 .color(theme.extended_palette().danger.strong.color)
                //                         } else {
                //                             style
                //                         }
                //                     })
                //                     .padding(8)
                //                 ]
                //                 .spacing(8)
                //                 .align_y(Alignment::Center),
                //             ]
                //             .spacing(12)
                //             .into()
                //         }
                //         page::Page::MeasurementRunning => todo!(),
                //     })
                //     .next_button(
                //         "Next",
                //         match self.page {
                //             page::Page::PortSetup => {
                //                 if self.selected_out_port.is_some()
                //                     && self.selected_in_port.is_some()
                //                 {
                //                     Some(Message::RunTest)
                //                 } else {
                //                     None
                //                 }
                //             }
                //             page::Page::LoudnessTest { loudness, .. } => {
                //                 if loudness.rms >= -14.0 && loudness.rms <= -10.0 {
                //                     Some(Message::TestOk)
                //                 } else {
                //                     None
                //                 }
                //             }
                //             page::Page::MeasurementSetup(ref fields) => {
                //                 let range = config::FrequencyRange::from_strings(
                //                     &fields.start_frequency,
                //                     &fields.end_frequency,
                //                 );
                //                 let duration = fields.duration.parse().map(Duration::from_secs_f32);
                //                 if let (Ok(range), Ok(duration)) = (range, duration) {
                //                     let config = measurement::Config::new(range, duration);
                //                     Some(Message::RunMeasurement(config))
                //                 } else {
                //                     None
                //                 }
                //             }
                //             page::Page::MeasurementRunning => todo!(),
                //         },
                //     )
                // self.page.view().content(match self.page {
                //     page::Page::LoudnessTest => todo!(),
                //     page::Page::MeasurementSetup => todo!(),
                //     page::Page::MeasurementRunning => todo!(),
                // })

                // let setup_page = |sample_rate, start_test_msg| -> Element<'_, Message> {
                //     Page::new("Setup")
                //         .content(
                //             row![
                //                 column![
                //                     text("Out port"),
                //                     pick_list(
                //                         backend.out_ports.as_slice(),
                //                         self.selected_out_port.as_ref(),
                //                         Message::OutPortSelected
                //                     )
                //                 ]
                //                 .spacing(6),
                //                 column![
                //                     text("In port"),
                //                     pick_list(
                //                         backend.in_ports.as_slice(),
                //                         self.selected_in_port.as_ref(),
                //                         Message::InPortSelected
                //                     )
                //                 ]
                //                 .spacing(6),
                //             ]
                //             .spacing(12),
                //         )
                //         .push_button(button("Cancel").on_press(Message::Back))
                //         .push_button(button("Start test").on_press_maybe(start_test_msg))
                //         .view(sample_rate)
                // };

                // fn loudness_text<'a>(label: &'a str, value: f32) -> Element<'a, Message> {
                //     column![
                //         text(label).size(12).align_y(Vertical::Bottom),
                //         horizontal_rule(1),
                //         text!("{:.1}", value).size(24),
                //     ]
                //     .spacing(3)
                //     .width(Length::Shrink)
                //     .align_x(Horizontal::Center)
                //     .into()
                // }

                // let sample_rate = &backend.sample_rate;
                // match measurement {
                //     MeasurementState::Init => setup_page(sample_rate, None),
                //     MeasurementState::ReadyForTest => {
                //         setup_page(sample_rate, Some(Message::RunTest))
                //     }
                //     MeasurementState::Testing { loudness, .. } => {
                //         Page::new("Loudness Test ...")
                //             .content(
                // row![
                //     container(
                //         canvas(RmsPeakMeter::new(
                //             loudness.rms,
                //             loudness.peak,
                //             &self.cache
                //         ))
                //         .width(60)
                //         .height(200)
                //     )
                //     .padding(10),
                //     column![
                //         container(
                //             row![
                //                 loudness_text("RMS", loudness.rms),
                //                 vertical_rule(3).style(|theme| {
                //                     let mut style = rule::default(theme);
                //                     style.width = 3;
                //                     style
                //                 }),
                //                 loudness_text("Peak", loudness.peak),
                //             ]
                //             .align_y(Vertical::Bottom)
                //             .height(Length::Shrink)
                //             .spacing(10)
                //         )
                //         .center_x(Length::Fill),
                //         slider(0.0..=1.0, self.volume, Message::VolumeChanged)
                //             .step(0.01),
                //     ]
                //     .spacing(10)
                // ]
                // .align_y(Vertical::Center),
                //             )
                //             .push_button(button("Stop").on_press(Message::StopTesting))
                //             .push_button(button("Ok").on_press(Message::TestOk))
                //             .view(sample_rate)
                //     }
                //     MeasurementState::PreparingMeasurement(fields) => {
                //         let range = config::FrequencyRange::from_strings(
                //             &fields.start_frequency,
                //             &fields.end_frequency,
                //         );
                //         let range_err = range.is_err();
                //         let duration = fields.duration.parse().map(Duration::from_secs_f32);
                //         let duration_err = duration.is_err();
                //         Page::new("Setup Measurement")
                //             .content(
                //                 column![
                //                     row![
                //                         column![
                //                             text("Out port"),
                //                             container(text!(
                //                                 "{}",
                //                                 self.selected_out_port.as_ref().unwrap()
                //                             ))
                //                             .padding(3)
                //                             .style(container::rounded_box)
                //                         ]
                //                         .spacing(6),
                //                         column![
                //                             text("In port"),
                //                             container(text!(
                //                                 "{}",
                //                                 self.selected_in_port.as_ref().unwrap()
                //                             ))
                //                             .padding(3)
                //                         ]
                //                         .spacing(6),
                //                     ]
                //                     .spacing(12),
                //                     row![
                //                         {
                //                             let color = move |theme: &iced::Theme| {
                //                                 if range_err {
                //                                     theme.extended_palette().danger.weak.color
                //                                 } else {
                //                                     theme
                //                                         .extended_palette()
                //                                         .secondary
                //                                         .strong
                //                                         .color
                //                                 }
                //                             };

                //                             container(
                //                                 column![text("Frequency"), horizontal_rule(1),]
                //                                     .push_maybe(
                //                                         range
                //                                             .as_ref()
                //                                             .err()
                //                                             .map(|err| text!("{err}")),
                //                                     )
                //                                     .push(
                //                                         row![
                //                                             text("From"),
                //                                             text_input(
                //                                                 "From",
                //                                                 &fields.start_frequency
                //                                             )
                //                                             .on_input(|s| {
                //                                                 Message::ConfigFieldChanged(
                //                                                     Field::StartFrequency(s),
                //                                                 )
                //                                             })
                //                                             .style(move |theme, status| {
                //                                                 let mut style =
                //                                                     text_input::default(
                //                                                         theme, status,
                //                                                     );
                //                                                 style.border = style
                //                                                     .border
                //                                                     .color(color(theme));
                //                                                 style
                //                                             }),
                //                                             text("To"),
                //                                             text_input(
                //                                                 "To",
                //                                                 &fields.end_frequency
                //                                             )
                //                                             .on_input(|s| {
                //                                                 Message::ConfigFieldChanged(
                //                                                     Field::EndFrequency(s),
                //                                                 )
                //                                             }),
                //                                         ]
                //                                         .spacing(8)
                //                                         .align_y(Alignment::Center),
                //                                     )
                //                                     .spacing(6),
                //                             )
                //                             .style(move |theme| {
                //                                 let style = container::rounded_box(theme);
                //                                 if range_err {
                //                                     style.color(color(theme))
                //                                 } else {
                //                                     style
                //                                 }
                //                             })
                //                             .padding(8)
                //                         },
                //                         container(row![
                //                             column![
                //                                 text("Duration"),
                //                                 horizontal_rule(1),
                //                                 text_input("Duration", &fields.duration)
                //                                     .on_input(|s| Message::ConfigFieldChanged(
                //                                         Field::Duration(s)
                //                                     ))
                //                                     .style(
                //                                         move |theme: &iced::Theme, status| {
                //                                             if duration_err {
                //                                                 text_input::Style {
                //                                                     border: iced::Border {
                //                                                         color: theme
                //                                                             .extended_palette()
                //                                                             .danger
                //                                                             .base
                //                                                             .color,
                //                                                         width: 1.0,
                //                                                         ..Default::default()
                //                                                     },
                //                                                     ..text_input::default(
                //                                                         theme, status,
                //                                                     )
                //                                                 }
                //                                             } else {
                //                                                 text_input::default(
                //                                                     theme, status,
                //                                                 )
                //                                             }
                //                                         }
                //                                     ),
                //                             ]
                //                             .spacing(8),
                //                             horizontal_space()
                //                         ])
                //                         .style(move |theme| {
                //                             let style = container::rounded_box(theme);
                //                             if duration_err {
                //                                 style.color(
                //                                     theme
                //                                         .extended_palette()
                //                                         .danger
                //                                         .strong
                //                                         .color,
                //                                 )
                //                             } else {
                //                                 style
                //                             }
                //                         })
                //                         .padding(8)
                //                     ]
                //                     .spacing(8)
                //                     .align_y(Alignment::Center),
                //                 ]
                //                 .spacing(12),
                //             )
                //             .push_button(button("Cancel").on_press(Message::Back))
                //             .push_button(button("Start Measurement").on_press_maybe(
                //                 if let (Ok(range), Ok(duration)) = (range, duration) {
                //                     let config = measurement::Config::new(range, duration);
                //                     Some(Message::RunMeasurement(config))
                //                 } else {
                //                     None
                //                 },
                //             ))
                //             .view(sample_rate)
                //     }
                //     MeasurementState::MeasurementRunning {
                //         loudness,
                //         data,
                //         finished_len,
                //     } => Page::new("Measurement Running ...")
                //         .content(
                //             row![
                //                 container(
                //                     canvas(RmsPeakMeter::new(
                //                         loudness.rms,
                //                         loudness.peak,
                //                         &self.cache
                //                     ))
                //                     .width(60)
                //                     .height(200)
                //                 )
                //                 .padding(10),
                //                 column![
                //                     container(
                //                         row![
                //                             loudness_text("RMS", loudness.rms),
                //                             vertical_rule(3).style(|theme| {
                //                                 let mut style = rule::default(theme);
                //                                 style.width = 3;
                //                                 style
                //                             }),
                //                             loudness_text("Peak", loudness.peak),
                //                         ]
                //                         .align_y(Vertical::Bottom)
                //                         .height(Length::Shrink)
                //                         .spacing(10)
                //                     )
                //                     .center_x(Length::Fill),
                //                     Chart::<_, (), _>::new()
                //                         .x_range(0.0..=*finished_len as f32)
                //                         .y_range(-0.5..=0.5)
                //                         .push_series(
                //                             line_series(
                //                                 data.iter()
                //                                     .enumerate()
                //                                     .map(|(i, s)| (i as f32, *s))
                //                             )
                //                             .color(
                //                                 iced::Color::from_rgb8(50, 175, 50)
                //                                     .scale_alpha(0.6)
                //                             )
                //                         )
                //                 ]
                //                 .spacing(12)
                //                 .padding(10)
                //             ]
                //             .spacing(12)
                //             .align_y(Vertical::Center),
                //         )
                //         .push_button(button("Stop").on_press(Message::StopTesting))
                //         .view(sample_rate),
                // }
            }
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

        Component::new("Loudness Test ...")
            .content(
                row![
                    container(
                        canvas(RmsPeakMeter::new(loudness.rms, loudness.peak, &self.cache))
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
                recording::Volume::new(self.volume, loudness)
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

pub fn recording_button<'a, Message: 'a>(msg: Message) -> Button<'a, Message> {
    button(colored_circle(8.0, Color::from_rgb8(200, 56, 42))).on_press(msg)
}
