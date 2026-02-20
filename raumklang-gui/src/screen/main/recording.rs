mod page;

use crate::{
    audio,
    data::{
        self,
        measurement::config,
        recording::{self, volume},
    },
    screen::main::recording::page::Component,
    widget::{RmsPeakMeter, meter},
};

use iced::{
    Alignment::Center,
    Element,
    Length::{Fill, Shrink},
    Subscription, Task,
    alignment::{Horizontal, Vertical},
    task, time,
    widget::{self, canvas, column, container, pick_list, row, rule, slider, text, text_input},
};
use prism::line_series;
use tokio_stream::wrappers::ReceiverStream;

use std::{fmt, sync::Arc, time::Duration};

#[derive(Debug)]
pub struct Recording {
    kind: Kind,
    state: State,
    volume: f32,
    backend: Backend,
    selected_in_port: Option<String>,
    selected_out_port: Option<String>,
    start_frequency: String,
    end_frequency: String,
    duration: String,
    cache: canvas::Cache,
}

#[derive(Debug, Default)]
pub enum State {
    #[default]
    Setup,
    LoudnessTest {
        signal_config: data::measurement::Config,
        loudness: audio::Loudness,
        _stream_handle: task::Handle,
    },
    Measurement(Measurement),
}

#[derive(Debug)]
pub struct Measurement {
    finished_len: usize,
    loudness: audio::Loudness,
    data: Vec<f32>,
    cache: canvas::Cache,
}

#[derive(Debug, Clone)]
pub enum Kind {
    Loopback,
    Measurement,
}

#[derive(Debug)]
enum Backend {
    NotConnected,
    Connected {
        backend: audio::Backend,
    },
    Retrying {
        err: audio::Error,
        end: std::time::Instant,
        remaining: std::time::Duration,
        retry_tx: std::sync::mpsc::SyncSender<()>,
    },
}

#[derive(Debug, Clone)]
pub enum Message {
    OutPortSelected(String),
    InPortSelected(String),
    StartFrequencyChanged(String),
    EndFrequencyChanged(String),
    DurationChanged(String),

    VolumeChanged(f32),
    TestOk(recording::Volume),
    RmsChanged(audio::Loudness),
    RunTest(data::measurement::Config),

    AudioBackend(audio::Event),
    RetryTick(time::Instant),
    JackNotification(audio::Notification),

    RecordingChunk(Box<[f32]>),
    RecordingFinished,

    Back,
    Cancel,
    RetryNow,
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
        let config = data::measurement::Config::default();
        Self {
            kind,
            state: State::Setup,
            backend: Backend::NotConnected,

            selected_out_port: None,
            selected_in_port: None,
            start_frequency: format!("{}", config.start_frequency()),
            end_frequency: format!("{}", config.end_frequency()),
            duration: format!("{}", config.duration().into_inner().as_secs()),

            volume: 0.5,

            cache: canvas::Cache::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::AudioBackend(event) => match event {
                audio::Event::Ready(backend, receiver) => {
                    self.backend = Backend::Connected { backend };

                    if let Some(receiver) = Arc::into_inner(receiver) {
                        Action::Task(
                            Task::stream(ReceiverStream::new(receiver))
                                .map(Message::JackNotification),
                        )
                    } else {
                        Action::None
                    }
                }
                audio::Event::Error {
                    err,
                    retry_tx,
                    retry_in,
                } => {
                    self.backend = Backend::Retrying {
                        err,
                        end: time::Instant::now() + retry_in,
                        remaining: retry_in,
                        retry_tx,
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
                let Backend::Connected { backend, .. } = &self.backend else {
                    return Action::None;
                };

                Action::Task(Task::future(backend.clone().connect_out_port(port)).discard())
            }
            Message::InPortSelected(port) => {
                let Backend::Connected { backend, .. } = &self.backend else {
                    return Action::None;
                };

                Action::Task(Task::future(backend.clone().connect_in_port(port)).discard())
            }
            Message::RetryTick(instant) => {
                let Backend::Retrying { end, remaining, .. } = &mut self.backend else {
                    return Action::None;
                };

                *remaining = *end - instant;

                Action::None
            }
            Message::VolumeChanged(volume) => {
                let Backend::Connected { backend, .. } = &self.backend else {
                    return Action::None;
                };

                self.volume = volume;

                Action::Task(Task::future(backend.clone().set_volume(volume)).discard())
            }
            Message::RmsChanged(new_loudness) => {
                let State::LoudnessTest { loudness, .. } = &mut self.state else {
                    return Action::None;
                };

                *loudness = new_loudness;
                self.cache.clear();

                Action::None
            }
            Message::RunTest(signal_config) => {
                let Backend::Connected { backend } = &mut self.backend else {
                    return Action::None;
                };

                // FIXME duration not used
                let duration = Duration::from_secs(3);
                let rms_receiver = backend.run_test(duration);

                let (recv, handle) = Task::stream(ReceiverStream::new(rms_receiver))
                    .map(Message::RmsChanged)
                    .abortable();

                let handle = handle.abort_on_drop();

                self.state = State::LoudnessTest {
                    signal_config,
                    loudness: audio::Loudness::default(),
                    _stream_handle: handle,
                };

                Action::Task(Task::batch([
                    Task::future(backend.clone().set_volume(self.volume)).discard(),
                    recv,
                ]))
            }
            Message::TestOk(_volume) => {
                let Backend::Connected { backend } = &self.backend else {
                    return Action::None;
                };

                let State::LoudnessTest {
                    signal_config,
                    _stream_handle,
                    // loudness,
                    //port_config,
                    ..
                } = std::mem::take(&mut self.state)
                else {
                    return Action::None;
                };

                let sample_rate = backend.sample_rate;
                let finished_len = data::Samples::from_duration(
                    signal_config.duration().into_inner(),
                    sample_rate,
                )
                .into();

                let (loudness_receiver, mut data_receiver) = backend.run_measurement(signal_config);

                let measurement_sipper = iced::task::sipper(async move |mut progress| {
                    while let Some(data) = data_receiver.recv().await {
                        progress.send(data).await;
                    }
                });

                let measurement = Measurement {
                    finished_len,
                    loudness: audio::Loudness::default(),
                    data: vec![],
                    cache: canvas::Cache::new(),
                };

                let task = Task::batch(vec![
                    Task::stream(ReceiverStream::new(loudness_receiver)).map(Message::RmsChanged),
                    Task::sip(measurement_sipper, Message::RecordingChunk, |_| {
                        Message::RecordingFinished
                    }),
                ]);

                self.state = State::Measurement(measurement);

                Action::Task(task)
            }
            // Message::Measurement(message) => {
            //     let State::Measurement(page) = &mut self.state else {
            //         return Action::None;
            //     };

            //     match page.update(message) {
            //         Some(measurement) => Action::Finished(match self.kind {
            //             Kind::Loopback => {
            //                 Result::Loopback(raumklang_core::Loopback::new(measurement))
            //             }
            //             Kind::Measurement => Result::Measurement(measurement),
            //         }),
            //         None => Action::None,
            //     }
            // }
            Message::RecordingChunk(chunk) => {
                if let State::Measurement(measurement) = &mut self.state {
                    measurement.data.extend_from_slice(&chunk);
                };

                Action::None
            }
            Message::RecordingFinished => {
                let Backend::Connected { backend } = &self.backend else {
                    return Action::None;
                };

                let State::Measurement(measurement) = &mut self.state else {
                    return Action::None;
                };

                let data = std::mem::take(&mut measurement.data);
                let measurement =
                    raumklang_core::Measurement::new(backend.sample_rate.into(), data);

                Action::Finished(match self.kind {
                    Kind::Loopback => Result::Loopback(raumklang_core::Loopback::new(measurement)),
                    Kind::Measurement => Result::Measurement(measurement),
                })
            }
            Message::Cancel => Action::Cancel,
            Message::Back => {
                let state = std::mem::take(&mut self.state);

                self.state = match state {
                    State::Setup => state,
                    State::LoudnessTest { .. } => State::Setup,
                    State::Measurement(_measurement) => State::Setup,
                };

                Action::None
            }
            Message::RetryNow => {
                let Backend::Retrying { retry_tx, .. } = &self.backend else {
                    return Action::None;
                };

                let _ = retry_tx.send(());

                Action::None
            }
            Message::StartFrequencyChanged(start) => {
                self.start_frequency = start;
                Action::None
            }
            Message::EndFrequencyChanged(end) => {
                self.end_frequency = end;
                Action::None
            }
            Message::DurationChanged(duration) => {
                self.duration = duration;
                Action::None
            }
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let page = match &self.backend {
            Backend::NotConnected => {
                page::Component::new("Jack").content(text("Jack is not connected."))
            }
            Backend::Connected { backend } => match &self.state {
                State::Setup => self.setup(backend),
                State::LoudnessTest { loudness, .. } => self.loudness_test(&loudness),
                State::Measurement(measurement) => self.measurement(measurement),
            },
            Backend::Retrying { err, remaining, .. } => self.retry(err, remaining),
        };

        let page = page.cancel_button("Cancel", Message::Cancel);

        container(page).width(600.0).into()
    }

    fn setup<'a>(&'a self, backend: &'a audio::Backend) -> page::Component<'a, Message> {
        let range =
            config::FrequencyRange::from_strings(&self.start_frequency, &self.end_frequency);

        let duration = config::Duration::from_string(&self.duration);

        let ports = {
            field_group(
                "Ports",
                column![
                    column![
                        text("Out"),
                        pick_list(
                            backend.out_ports.as_slice(),
                            self.selected_out_port.as_ref(),
                            Message::OutPortSelected
                        )
                        .style(|t, s| {
                            let mut base = pick_list::default(t, s);
                            base.background =
                                iced::Background::Color(t.extended_palette().background.base.color);
                            base
                        })
                    ]
                    .spacing(6),
                    column![
                        text("In"),
                        pick_list(
                            backend.in_ports.as_slice(),
                            self.selected_in_port.as_ref(),
                            Message::InPortSelected
                        )
                        .style(|t, s| {
                            let mut base = pick_list::default(t, s);
                            base.background =
                                iced::Background::Color(t.extended_palette().background.base.color);
                            base
                        })
                    ]
                    .spacing(6),
                ]
                .spacing(12),
                None::<&String>,
            )
        };

        let signal = {
            column![
                field_group(
                    "Frequency",
                    row![
                        number_input(&self.start_frequency, range.is_ok())
                            .label("From")
                            .unit("Hz")
                            .on_input(Message::StartFrequencyChanged),
                        number_input(&self.end_frequency, range.is_ok())
                            .label("To")
                            .unit("Hz")
                            .on_input(Message::EndFrequencyChanged)
                    ]
                    .spacing(8)
                    .align_y(Center),
                    range.as_ref().err()
                ),
                field_group(
                    "Duration",
                    number_input(&self.duration, duration.is_ok())
                        .unit("s")
                        .on_input(Message::DurationChanged),
                    duration.as_ref().err()
                )
            ]
            .spacing(8)
        };

        let ports_selected = self
            .selected_out_port
            .as_ref()
            .and(self.selected_in_port.as_ref());

        let signal_config = if let (Ok(range), Ok(duration)) = (range, duration) {
            Some(data::measurement::Config::new(range, duration))
        } else {
            None
        };

        Component::new("Setup")
            .content(row![ports, signal].spacing(8))
            .next_button(
                "Start test",
                ports_selected.and(signal_config).map(Message::RunTest),
            )
    }

    fn loudness_test(&self, loudness: &audio::Loudness) -> page::Component<'_, Message> {
        fn loudness_text<'a>(label: &'a str, value: f32) -> Element<'a, Message> {
            column![
                text(label).size(12).align_y(Vertical::Bottom),
                rule::horizontal(1),
                text!("{:.1}", value).size(24),
            ]
            .spacing(3)
            .width(Shrink)
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
                                rule::vertical(3),
                                loudness_text("Peak", loudness.peak),
                            ]
                            .align_y(Vertical::Bottom)
                            .height(Shrink)
                            .spacing(10)
                        )
                        .center_x(Fill),
                        slider(0.0..=1.0, self.volume, Message::VolumeChanged).step(0.01),
                    ]
                    .spacing(10)
                ]
                .align_y(Vertical::Center),
            )
            .next_button("Next", volume.ok().map(Message::TestOk))
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let audio_backend = Subscription::run(audio::run).map(Message::AudioBackend);

        let mut subscriptions = vec![audio_backend];

        if let Backend::Retrying { .. } = &self.backend {
            subscriptions.push(time::every(Duration::from_millis(500)).map(Message::RetryTick));
        }

        Subscription::batch(subscriptions)
    }

    fn measurement<'a>(&self, measurement: &'a Measurement) -> Component<'a, Message> {
        Component::new("Measurement Running ...").content(
            row![
                container(
                    canvas(RmsPeakMeter::new(
                        measurement.loudness.rms,
                        measurement.loudness.peak,
                        &measurement.cache
                    ))
                    .width(60)
                    .height(200)
                )
                .padding(10),
                column![
                    container(
                        row![
                            loudness_text("RMS", measurement.loudness.rms),
                            rule::vertical(3),
                            loudness_text("Peak", measurement.loudness.peak),
                        ]
                        .align_y(Vertical::Bottom)
                        .height(Shrink)
                        .spacing(10)
                    )
                    .center_x(Fill),
                    // TODO replace with waveform
                    prism::Chart::<_, (), _>::new()
                        .x_range(0.0..=measurement.finished_len as f32)
                        .y_range(-0.5..=0.5)
                        .push_series(
                            line_series(
                                measurement
                                    .data
                                    .iter()
                                    .enumerate()
                                    .map(|(i, s)| (i as f32, *s))
                            )
                            .color(iced::Color::from_rgb8(50, 175, 50).scale_alpha(0.6))
                        )
                ]
                .spacing(12)
                .padding(10)
            ]
            .spacing(12)
            .align_y(Vertical::Center),
        )
    }

    fn retry(&self, err: &audio::Error, remaining: &Duration) -> page::Component<'_, Message> {
        page::Component::new("Jack error")
            .content(
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
                .center_x(Fill),
            )
            .next_button("Retry now", Some(Message::RetryNow))
    }
}

impl Default for Recording {
    fn default() -> Self {
        Self::new(Kind::Measurement)
    }
}

fn field_group<'a, Message>(
    label: &'a str,
    content: impl Into<Element<'a, Message>>,
    err: Option<&impl fmt::Display>,
) -> Element<'a, Message>
where
    Message: 'a,
{
    container(
        column![text(label), rule::horizontal(1),]
            .push(column!().push(err.map(|err| {
                text!("{}", err).style(|theme| {
                    let mut style = text::default(theme);
                    style.color = Some(theme.extended_palette().danger.base.color);
                    style
                })
            })))
            .push(content)
            .spacing(6),
    )
    .style(container::rounded_box)
    .padding(8)
    .into()
}
fn number_input<'a, Message>(value: &'a str, is_valid: bool) -> NumberInput<'a, Message>
where
    Message: 'a + Clone,
{
    NumberInput::new(value, is_valid)
}

struct NumberInput<'a, Message> {
    label: Option<&'a str>,
    value: &'a str,
    unit: Option<&'a str>,
    is_valid: bool,
    on_input: Option<Box<dyn Fn(String) -> Message + 'a>>,
}

impl<'a, Message> NumberInput<'a, Message>
where
    Message: 'a + Clone,
{
    fn new(value: &'a str, is_valid: bool) -> Self {
        Self {
            label: None,
            value,
            unit: None,
            is_valid,
            on_input: None,
        }
    }

    fn label(mut self, label: &'a str) -> Self {
        self.label = Some(label);
        self
    }

    fn unit(mut self, unit: &'a str) -> Self {
        self.unit = Some(unit);
        self
    }

    fn on_input(mut self, on_input: impl Fn(String) -> Message + 'a) -> Self {
        self.on_input = Some(Box::new(on_input));
        self
    }

    fn view(self) -> Element<'a, Message> {
        column![]
            .push(self.label.map(text))
            .push(
                row![
                    text_input("", self.value)
                        .id(widget::Id::new("from"))
                        .align_x(Horizontal::Right)
                        .on_input_maybe(self.on_input)
                        .style(if self.is_valid {
                            text_input::default
                        } else {
                            number_input_danger
                        })
                ]
                .push(self.unit.map(text))
                .align_y(Vertical::Center)
                .spacing(3),
            )
            .into()
    }
}

fn number_input_danger(theme: &iced::Theme, status: text_input::Status) -> text_input::Style {
    let danger = theme.extended_palette().danger;

    let mut style = text_input::default(theme, status);

    let color = match status {
        text_input::Status::Active => danger.base.color,
        text_input::Status::Hovered => danger.strong.color,
        text_input::Status::Focused { is_hovered: _ } => danger.strong.color,
        text_input::Status::Disabled => danger.weak.color,
    };

    style.border = style.border.color(color);
    style
}

impl<'a, Message> From<NumberInput<'a, Message>> for Element<'a, Message>
where
    Message: 'a + Clone,
{
    fn from(number_input: NumberInput<'a, Message>) -> Self {
        number_input.view()
    }
}

fn loudness_text<'a>(label: &'a str, value: f32) -> Element<'a, Message> {
    column![
        text(label).size(12).align_y(Vertical::Bottom),
        rule::horizontal(1),
        text!("{:.1}", value).size(24),
    ]
    .spacing(3)
    .width(Shrink)
    .align_x(Center)
    .into()
}
