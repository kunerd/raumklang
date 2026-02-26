use crate::{
    audio,
    data::{
        self, SampleRate,
        audio::{InPort, OutPort},
        measurement::{self, config},
        recording::{self, volume},
    },
    log,
    screen::main::chart::{self},
    widget::{RmsPeakMeter, meter},
};

use iced::{
    Alignment::Center,
    Element,
    Length::{Fill, Shrink},
    Subscription, Task,
    alignment::{Horizontal, Vertical},
    task, time,
    widget::{
        self, Button, button, canvas, center, column, container, pick_list, right, row, rule,
        slider, space, text, text_input,
    },
};
use tokio_stream::wrappers::ReceiverStream;

use std::{fmt, sync::Arc, time::Duration};

#[derive(Debug)]
pub struct Recording {
    kind: Kind,
    state: State,
    volume: f32,
    backend: Backend,
    selected_in_port: Option<InPort>,
    selected_out_port: Option<OutPort>,
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
        config: measurement::SignalConfig,
        loudness: audio::Loudness,
        _stream_handle: task::Handle,
    },
    Measurement(Measurement),
}

#[derive(Debug)]
pub struct Measurement {
    loudness: audio::Loudness,

    data: Vec<f32>,

    config: measurement::SignalConfig,

    finished: bool,
    cache: canvas::Cache,
    _stream_handle: task::Handle,
}

#[derive(Debug, Clone)]
pub enum Kind {
    Loopback,
    Measurement,
}

#[derive(Debug)]
enum Backend {
    Connecting(Option<Retry>),
    Connected { backend: audio::Backend },
}

#[derive(Debug)]
struct Retry {
    err: audio::Error,
    end: std::time::Instant,
    remaining: std::time::Duration,
    retry_tx: std::sync::mpsc::SyncSender<()>,
}

#[derive(Debug, Clone)]
pub enum Message {
    OutPortSelected(OutPort),
    InPortSelected(InPort),
    StartFrequencyChanged(String),
    EndFrequencyChanged(String),
    DurationChanged(String),

    VolumeChanged(f32),
    TestOk(recording::Volume),
    RmsChanged(audio::Loudness),
    RunTest(data::measurement::SignalConfig),

    AudioBackend(audio::Event),
    RetryTick(time::Instant),
    JackNotification(audio::Notification),

    RecordingChunk(Box<[f32]>),
    RecordingFinished,

    Chart(()),

    Back,
    Cancel,
    RetryNow,
    Decline,
    Accept,
}

pub enum Action {
    None,
    Cancel,
    Task(Task<Message>),
    Finished(measurement::Config, Result),
}

pub enum Result {
    Loopback(raumklang_core::Loopback),
    Measurement(raumklang_core::Measurement),
}

impl Recording {
    pub fn new(kind: Kind, config: measurement::Config) -> Self {
        Self {
            kind,
            state: State::Setup,
            backend: Backend::Connecting(None),

            selected_in_port: config.in_port,
            selected_out_port: config.out_port,

            start_frequency: format!("{}", config.signal.start_frequency()),
            end_frequency: format!("{}", config.signal.end_frequency()),
            duration: format!("{}", config.signal.duration().into_inner().as_secs()),

            volume: 0.5,

            cache: canvas::Cache::new(),
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::AudioBackend(event) => match event {
                audio::Event::Ready(backend, receiver) => {
                    if let Some(receiver) = Arc::into_inner(receiver) {
                        let mut tasks = vec![
                            Task::stream(ReceiverStream::new(receiver))
                                .map(Message::JackNotification),
                        ];

                        if let Some(port) = self.selected_out_port.as_ref() {
                            tasks.push(
                                Task::future(backend.clone().connect_out_port(port.clone()))
                                    .discard(),
                            )
                        }

                        if let Some(port) = self.selected_in_port.as_ref() {
                            tasks.push(
                                Task::future(backend.clone().connect_in_port(port.clone()))
                                    .discard(),
                            );
                        }

                        self.backend = Backend::Connected { backend };

                        Action::Task(Task::batch(tasks))
                    } else {
                        Action::None
                    }
                }
                audio::Event::Error {
                    err,
                    retry_tx,
                    retry_in,
                } => {
                    self.backend = Backend::Connecting(Some(Retry {
                        err,
                        end: time::Instant::now() + retry_in,
                        remaining: retry_in,
                        retry_tx,
                    }));
                    Action::None
                }
            },
            Message::JackNotification(notification) => {
                // TODO: currently, selected ports will be erased when the jack
                // server is closed (jack will disconnect them)
                // it would be nice to have a way to restore them
                match notification {
                    audio::Notification::OutPortConnected(port) => {
                        log::debug!("out port {port} connected");
                        self.selected_out_port = Some(port)
                    }
                    audio::Notification::OutPortDisconnected => {
                        log::debug!("out port disconnected");
                        self.selected_out_port = None
                    }
                    audio::Notification::InPortConnected(port) => {
                        log::debug!("in port {port} connected");
                        self.selected_in_port = Some(port)
                    }
                    audio::Notification::InPortDisconnected => {
                        log::debug!("in port disconnected");
                        self.selected_in_port = None
                    }
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
                let Backend::Connecting(Some(retry)) = &mut self.backend else {
                    return Action::None;
                };

                retry.remaining = retry.end - instant;

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
                if let State::LoudnessTest { loudness, .. } = &mut self.state {
                    *loudness = new_loudness;
                    self.cache.clear();
                }

                if let State::Measurement(measurement) = &mut self.state {
                    measurement.loudness = new_loudness;
                    self.cache.clear();
                }

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
                    config: signal_config,
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
                    config,
                    _stream_handle,
                    ..
                } = std::mem::take(&mut self.state)
                else {
                    return Action::None;
                };

                let (loudness_receiver, mut data_receiver) =
                    backend.run_measurement(config.clone());

                let measurement_sipper = iced::task::sipper(async move |mut progress| {
                    while let Some(data) = data_receiver.recv().await {
                        progress.send(data).await;
                    }
                });

                let (sipper, handle) =
                    Task::sip(measurement_sipper, Message::RecordingChunk, |_| {
                        Message::RecordingFinished
                    })
                    .abortable();

                let measurement = Measurement {
                    loudness: audio::Loudness::default(),
                    data: vec![],
                    cache: canvas::Cache::new(),
                    _stream_handle: handle,
                    finished: false,
                    config,
                };

                let task = Task::batch(vec![
                    Task::stream(ReceiverStream::new(loudness_receiver)).map(Message::RmsChanged),
                    sipper,
                ]);

                self.state = State::Measurement(measurement);

                Action::Task(task)
            }
            Message::RecordingChunk(chunk) => {
                if let State::Measurement(measurement) = &mut self.state {
                    measurement.data.extend_from_slice(&chunk);
                    measurement.cache.clear();
                };

                Action::None
            }
            Message::RecordingFinished => {
                if let State::Measurement(measurement) = &mut self.state {
                    measurement.finished = true;
                };
                Action::None
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
                let Backend::Connecting(Some(retry)) = &self.backend else {
                    return Action::None;
                };

                let _ = retry.retry_tx.send(());

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
            Message::Chart(_interaction) => {
                // no interaction needed at this point
                Action::None
            }
            Message::Decline => {
                self.state = State::Setup;
                Action::None
            }
            Message::Accept => {
                let Backend::Connected { backend } = &self.backend else {
                    return Action::None;
                };

                let State::Measurement(measurement) = std::mem::take(&mut self.state) else {
                    return Action::None;
                };

                let signal = measurement.data;
                let signal = raumklang_core::Measurement::new(backend.sample_rate.into(), signal);
                let result = match self.kind {
                    Kind::Loopback => Result::Loopback(raumklang_core::Loopback::new(signal)),
                    Kind::Measurement => Result::Measurement(signal),
                };

                let config = measurement::Config {
                    out_port: self.selected_out_port.take(),
                    in_port: self.selected_in_port.take(),
                    signal: measurement.config,
                };

                Action::Finished(config, result)
            }
        }
    }

    pub fn view<'a>(&'a self) -> Element<'a, Message> {
        let page = match &self.backend {
            Backend::Connecting(retry) => self.retry(retry.as_ref()),
            Backend::Connected { backend } => match &self.state {
                State::Setup => self.setup(backend),
                State::LoudnessTest { loudness, .. } => {
                    self.loudness_test(&loudness, backend.sample_rate)
                }
                State::Measurement(measurement) => {
                    self.measurement(measurement, backend.sample_rate)
                }
            },
        };

        container(page).width(600.0).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        let audio_backend = Subscription::run(audio::run).map(Message::AudioBackend);

        let mut subscriptions = vec![audio_backend];

        if let Backend::Connecting(..) = &self.backend {
            subscriptions.push(time::every(Duration::from_millis(500)).map(Message::RetryTick));
        }

        Subscription::batch(subscriptions)
    }

    fn setup<'a>(&'a self, backend: &'a audio::Backend) -> Element<'a, Message> {
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
                            self.selected_out_port.as_ref(),
                            backend.out_ports.as_slice(),
                            OutPort::to_string
                        )
                        .on_select(Message::OutPortSelected)
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
                            self.selected_in_port.as_ref(),
                            backend.in_ports.as_slice(),
                            InPort::to_string
                        )
                        .on_select(Message::InPortSelected)
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
            Some(data::measurement::SignalConfig::new(range, duration))
        } else {
            None
        };

        let start_btn = button("Start")
            .style(button::success)
            .on_press_maybe(ports_selected.and(signal_config).map(Message::RunTest));

        page(
            "Setup",
            Some(backend.sample_rate),
            row![ports, signal].spacing(8),
            button("Cancel")
                .style(button::danger)
                .on_press(Message::Cancel),
            None,
            Some(start_btn),
        )
    }

    fn loudness_test(
        &self,
        loudness: &audio::Loudness,
        sample_rate: SampleRate,
    ) -> Element<'_, Message> {
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

        let content = row![
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
        .align_y(Vertical::Center);

        let next_btn = button("Next")
            .style(button::success)
            .on_press_maybe(volume.ok().map(Message::TestOk));

        page(
            "Loudness test ...",
            Some(sample_rate),
            content,
            button("Cancel")
                .style(button::danger)
                .on_press(Message::Cancel),
            Some(
                button("Stop")
                    .style(button::secondary)
                    .on_press(Message::Back),
            ),
            Some(next_btn),
        )
    }

    fn measurement<'a>(
        &'a self,
        measurement: &'a Measurement,
        sample_rate: SampleRate,
    ) -> Element<'a, Message> {
        let title = match measurement.finished {
            true => "Measurement running ...",
            false => "Measurement finished",
        };

        let content = row![
            container(
                canvas(RmsPeakMeter::new(
                    measurement.loudness.rms,
                    measurement.loudness.peak,
                    &self.cache
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
                center(
                    chart::record_waveform(sample_rate, &measurement.data, &measurement.cache)
                        .map(Message::Chart),
                )
            ]
            .height(500)
            .spacing(12)
            .padding(10)
        ]
        .spacing(12)
        .align_y(Vertical::Center);

        let back_btn = {
            let (title, msg) = match measurement.finished {
                true => ("Decline", Message::Decline),
                false => ("Stop", Message::Back),
            };
            button(title).style(button::danger).on_press(msg)
        };

        page(
            &title,
            Some(sample_rate),
            content,
            button("Cancel")
                .style(button::danger)
                .on_press(Message::Cancel),
            Some(back_btn),
            Some(
                button("Accept")
                    .style(button::success)
                    .on_press_maybe(measurement.finished.then_some(Message::Accept)),
            ),
        )
    }

    fn retry(&self, retry: Option<&Retry>) -> Element<'_, Message> {
        match retry {
            Some(retry) => {
                let content = container(
                    column![
                        text("Connection to Jack audio server failed:")
                            .size(18)
                            .style(text::danger),
                        text!("{}", retry.err).style(text::danger),
                        column![
                            text("Retrying in").size(14),
                            text!("{} s", retry.remaining.as_secs()).size(18)
                        ]
                        .padding(8)
                        .align_x(Horizontal::Center),
                    ]
                    .align_x(Horizontal::Center)
                    .spacing(16),
                )
                .center_x(Fill);

                page(
                    "Jack connection error",
                    None,
                    content,
                    button("Cancel")
                        .style(button::secondary)
                        .on_press(Message::Cancel),
                    None,
                    Some(
                        button("Retry now")
                            .style(button::secondary)
                            .on_press(Message::RetryNow),
                    ),
                )
            }
            None => page(
                "Connecting to Jack",
                None,
                center(text("Trying to connect to Jack audio server.")),
                button("Cancel")
                    .style(button::secondary)
                    .on_press(Message::Cancel),
                None,
                None,
            ),
        }
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

fn page<'a, Message>(
    title: &'a str,
    sample_rate: Option<SampleRate>,
    content: impl Into<Element<'a, Message>>,
    cancel: Button<'a, Message>,
    back: Option<Button<'a, Message>>,
    success: Option<Button<'a, Message>>,
) -> Element<'a, Message>
where
    Message: Clone + 'a,
{
    let header = {
        column![
            row![text!("Recording - {title}").size(20), space::horizontal(),]
                .push(sample_rate.map(|sample_rate| text!("Sample rate: {}", sample_rate).size(14)))
                .align_y(Vertical::Bottom),
            rule::horizontal(1),
        ]
        .spacing(4)
    };

    let footer = container(row![cancel, right(row![success, back].spacing(6))]).align_right(Fill);

    container(column![header, content.into(), footer].spacing(18))
        .style(container::bordered_box)
        .padding(18)
        .into()
}
