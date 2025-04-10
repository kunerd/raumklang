use std::time::Duration;

use crate::widgets::colored_circle;
use audio_backend::AudioBackend;
use iced::{
    alignment::Vertical,
    time,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, pick_list, row, slider, text,
        Button,
    },
    Color, Element, Length, Subscription, Task,
};
use tokio::sync::mpsc;
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
        backend: AudioBackend,
        measurement: MeasurementState,
    },
    Retrying {
        err: Option<audio_backend::Error>,
        end: std::time::Instant,
        remaining: std::time::Duration,
    },
    Error(audio_backend::Error),
}

enum MeasurementState {
    Init,
    ReadyForTest,
    Testing {
        loudness: audio_backend::Loudness,
        volume: mpsc::Sender<f32>,
    }, //     Testing,
       //     ReadyForMeasurement,
       //     MeasurementRunning,
       //     Done,
}

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    OutPortSelected(String),
    InPortSelected(String),
    RunTest,
    AudioBackend(audio_backend::Event),
    RetryTick(time::Instant),
    VolumeChanged(f32),
    StopTesting,
    RmsChanged(audio_backend::Loudness),
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

    pub fn update(&mut self, message: Message) -> Action {
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
                let (rms_receiver, volume_sender) = backend.run_test(duration);

                let _ = volume_sender.try_send(self.volume);
                *measurement = MeasurementState::Testing {
                    loudness: audio_backend::Loudness::default(),
                    volume: volume_sender,
                };

                Action::Task(
                    Task::stream(ReceiverStream::new(rms_receiver)).map(Message::RmsChanged),
                )
            }
            Message::AudioBackend(event) => match event {
                audio_backend::Event::Ready(backend) => {
                    self.state = State::Connected {
                        backend,
                        measurement: MeasurementState::Init,
                    };
                    Action::None
                }
                audio_backend::Event::Error(err) => {
                    println!("{err}");
                    self.state = State::Error(err);
                    Action::None
                }
                audio_backend::Event::RetryIn(timeout) => {
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
                audio_backend::Event::Notification(notification) => {
                    let State::Connected { .. } = self.state else {
                        return Action::None;
                    };

                    dbg!(&notification);
                    match notification {
                        audio_backend::Notification::OutPortConnected(port) => {
                            self.selected_out_port = Some(port)
                        }
                        audio_backend::Notification::OutPortDisconnected(_) => {
                            self.selected_out_port = None
                        }
                        audio_backend::Notification::InPortConnected(port) => {
                            self.selected_in_port = Some(port)
                        }
                        audio_backend::Notification::InPortDisconnected(_) => {
                            self.selected_in_port = None
                        }
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
                let State::Connected {
                    measurement:
                        MeasurementState::Testing {
                            volume: volume_sender,
                            ..
                        },
                    ..
                } = &mut self.state
                else {
                    return Action::None;
                };

                if let Ok(_) = volume_sender.try_send(volume) {
                    self.volume = volume;
                }

                Action::None
            }
            Message::StopTesting => {
                let State::Connected {
                    backend,
                    measurement: measurement @ MeasurementState::Testing { .. },
                } = &mut self.state
                else {
                    return Action::None;
                };

                backend.stop_test();
                *measurement = MeasurementState::ReadyForTest;

                Action::None
            }
            Message::RmsChanged(new_loudness) => {
                let State::Connected {
                    measurement: MeasurementState::Testing { loudness, .. },
                    ..
                } = &mut self.state
                else {
                    return Action::None;
                };

                *loudness = new_loudness;

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
                    let header = column![
                        row![
                            text("Recording").size(24),
                            horizontal_space(),
                            text!("Sample rate: {}", backend.sample_rate).size(14)
                        ]
                        .align_y(Vertical::Bottom),
                        horizontal_rule(1),
                    ]
                    .spacing(4);

                    match measurement {
                        MeasurementState::Init => container(
                            column![
                                header,
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
                                header,
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
                            text("Test running ..."),
                            text!("RMS: {}, Peak: {}", loudness.rms, loudness.peak),
                            slider(0.0..=1.0, self.volume, Message::VolumeChanged).step(0.01),
                            button("Stop").on_press(Message::StopTesting)
                        ])
                        .center_x(Length::Fill)
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
        let audio_backend = Subscription::run(audio_backend::run).map(Message::AudioBackend);

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
        }
    }
}

pub fn recording_button<'a, Message: 'a>(msg: Message) -> Button<'a, Message> {
    button(colored_circle(8.0, Color::from_rgb8(200, 56, 42))).on_press(msg)
}

mod audio_backend {

    use std::sync::atomic::AtomicBool;
    use std::sync::mpsc::RecvTimeoutError;
    use std::sync::Arc;
    use std::time::{Duration, Instant};

    use iced::futures::{SinkExt, Stream};
    use iced::stream;
    use jack::PortFlags;
    use raumklang_core::{dbfs, LoudnessMeter};
    use ringbuf::traits::{Consumer, Producer, Split};
    use ringbuf::{HeapCons, HeapProd, HeapRb};
    use tokio::sync::mpsc;
    use tokio::sync::mpsc::error::TryRecvError;

    use crate::data::{self};

    #[derive(Debug, Clone)]
    pub enum Event {
        Ready(AudioBackend),
        Notification(Notification),
        Error(Error),
        RetryIn(Duration),
    }

    #[derive(Debug, Clone)]
    pub enum Notification {
        OutPortConnected(String),
        OutPortDisconnected(String),
        InPortConnected(String),
        InPortDisconnected(String),
    }

    #[derive(Debug, Clone)]
    pub struct AudioBackend {
        pub sample_rate: data::SampleRate,
        pub in_ports: Vec<String>,
        pub out_ports: Vec<String>,
        sender: mpsc::Sender<Command>,
    }

    #[derive(Debug, Clone, Copy)]
    pub struct Loudness {
        pub rms: f32,
        pub peak: f32,
    }

    impl Default for Loudness {
        fn default() -> Self {
            Self {
                rms: f32::NEG_INFINITY,
                peak: f32::NEG_INFINITY,
            }
        }
    }

    impl AudioBackend {
        pub fn run_test(
            &self,
            duration: Duration,
        ) -> (mpsc::Receiver<Loudness>, mpsc::Sender<f32>) {
            let (loudness_sender, loudness_receiver) = mpsc::channel(100);
            let (volume_sender, volume_receiver) = mpsc::channel(100);

            let command = Command::RunTest {
                duration,
                loudness: loudness_sender,
                volume: volume_receiver,
            };

            self.sender.try_send(command);

            (loudness_receiver, volume_sender)
        }

        pub async fn connect_out_port(self, dest_port: String) {
            let command = Command::ConnectOutPort(dest_port);

            let _ = self.sender.send(command).await;
        }

        pub async fn connect_in_port(self, src_port: String) {
            let command = Command::ConnectInPort(src_port);

            let _ = self.sender.send(command).await;
        }

        pub fn stop_test(&self) {
            let _ = self.sender.try_send(Command::Stop);
        }
    }

    pub fn run() -> impl Stream<Item = Event> {
        stream::channel(100, async |mut output| {
            let (sender, mut receiver) = mpsc::channel(100);

            std::thread::spawn(|| run_audio_backend(sender));

            while let Some(event) = receiver.recv().await {
                let _ = output.send(event).await;
            }
        })
    }

    enum Command {
        RunTest {
            duration: Duration,
            loudness: mpsc::Sender<Loudness>,
            volume: mpsc::Receiver<f32>,
        },
        ConnectOutPort(String),
        ConnectInPort(String),
        Stop,
    }

    enum State {
        NotConnected(u64),
        Connected {
            client: jack::AsyncClient<Notifications, ProcessHandler>,
            commands: mpsc::Receiver<Command>,
            events: std::sync::mpsc::Receiver<Notification>,
            is_server_shutdown: Arc<AtomicBool>,
            process: std::sync::mpsc::SyncSender<ProcessHandlerMessage>,
        },
        Error(u64),
    }

    fn run_audio_backend(sender: mpsc::Sender<Event>) {
        let mut state = State::NotConnected(0);

        loop {
            match state {
                State::NotConnected(retry_count) => {
                    let is_server_shutdown = Arc::new(AtomicBool::new(false));
                    let (notify_sender, events_receiver) = std::sync::mpsc::sync_channel(64);

                    match start_jack_client(notify_sender, is_server_shutdown.clone()) {
                        Ok((client, process_sender)) => {
                            let sample_rate = client.as_client().sample_rate().into();
                            let out_ports = client.as_client().ports(
                                None,
                                Some("32 bit float mono audio"),
                                PortFlags::IS_INPUT,
                            );
                            let in_ports = client.as_client().ports(
                                None,
                                Some("32 bit float mono audio"),
                                PortFlags::IS_OUTPUT,
                            );

                            let (command_sender, command_receiver) = mpsc::channel(64);
                            let backend = AudioBackend {
                                sample_rate,
                                in_ports,
                                out_ports,
                                sender: command_sender,
                            };
                            let _ = sender.blocking_send(Event::Ready(backend));

                            state = State::Connected {
                                client,
                                commands: command_receiver,
                                events: events_receiver,
                                process: process_sender,
                                is_server_shutdown,
                            };
                        }
                        Err(err) => {
                            let _ = sender.blocking_send(Event::Error(err));
                            state = State::Error(retry_count);
                        }
                    }
                }
                State::Connected {
                    client,
                    mut commands,
                    events,
                    process,
                    is_server_shutdown,
                } => {
                    enum WorkerState {
                        Idle,
                        LoudnessTest(LoudnessTest),
                    }
                    struct LoudnessTest {
                        last_rms: Instant,
                        last_peak: Instant,
                        meter: LoudnessMeter,
                        signal: HeapProd<f32>,
                        signal_iter: std::iter::Take<raumklang_core::PinkNoise>,
                        recording: HeapCons<f32>,
                        volume: f32,
                        volume_receiver: mpsc::Receiver<f32>,
                        sender: mpsc::Sender<Loudness>,
                    }
                    let mut worker_state = WorkerState::Idle;

                    while is_server_shutdown.load(std::sync::atomic::Ordering::Relaxed) != true {
                        match events.recv_timeout(Duration::from_millis(10)) {
                            Ok(event) => {
                                let _ = sender.blocking_send(Event::Notification(event));
                            }
                            Err(RecvTimeoutError::Disconnected) => {
                                break;
                            }
                            Err(RecvTimeoutError::Timeout) => {
                                if sender.is_closed() {
                                    // their is no receiver anymore
                                    return;
                                }
                            }
                        }

                        match commands.try_recv() {
                            Ok(Command::ConnectOutPort(dest)) => {
                                if let Some(out_port) =
                                    client.as_client().port_by_name("gui:measurement_out")
                                {
                                    client.as_client().disconnect(&out_port);
                                }

                                client
                                    .as_client()
                                    .connect_ports_by_name("gui:measurement_out", &dest);
                            }
                            Ok(Command::ConnectInPort(source)) => {
                                if let Some(in_port) =
                                    client.as_client().port_by_name("gui:measurement_in")
                                {
                                    client.as_client().disconnect(&in_port);
                                }

                                client
                                    .as_client()
                                    .connect_ports_by_name(&source, "gui:measurement_in")
                                    .unwrap();
                            }
                            Ok(Command::RunTest {
                                duration,
                                loudness,
                                mut volume,
                            }) => {
                                let last_rms = Instant::now();
                                let last_peak = Instant::now();
                                // FIXME hardcoded sample rate dependency
                                let meter = LoudnessMeter::new(13230); // 44100samples / 1000ms * 300ms

                                let buf_size = client.as_client().buffer_size() as usize;
                                let (signal_prod, signal_cons) = HeapRb::new(buf_size).split();
                                let (recording_prod, recording_cons) =
                                    HeapRb::new(buf_size).split();

                                let process_msg = ProcessHandlerMessage::LoudnessMeasurement {
                                    in_buf: signal_cons,
                                    out_buf: recording_prod,
                                };

                                process.send(process_msg);

                                let test = LoudnessTest {
                                    last_rms,
                                    last_peak,
                                    meter,
                                    signal_iter: raumklang_core::PinkNoise::with_amplitude(0.8)
                                        .take_duration(
                                            44_100,
                                            data::Samples::from_duration(
                                                duration,
                                                data::SampleRate::new(44_100),
                                            )
                                            .into(),
                                        ),
                                    signal: signal_prod,
                                    recording: recording_cons,
                                    sender: loudness,
                                    volume: volume.try_recv().unwrap_or(0.5),
                                    volume_receiver: volume,
                                };
                                worker_state = WorkerState::LoudnessTest(test);
                            }
                            Ok(Command::Stop) => {
                                process.send(ProcessHandlerMessage::Stop);
                            }
                            Err(TryRecvError::Disconnected) => {
                                // their is no receiver anymore
                                return;
                            }
                            Err(TryRecvError::Empty) => {}
                        }

                        match worker_state {
                            WorkerState::Idle => {}
                            WorkerState::LoudnessTest(ref mut test) => {
                                if let Ok(vol) = test.volume_receiver.try_recv() {
                                    test.volume = vol;
                                }
                                let iter = &mut test.signal_iter;
                                test.signal.push_iter(
                                    iter.map(|s| {
                                        s * raumklang_core::volume_to_amplitude(test.volume)
                                    }),
                                );

                                let iter = test.recording.pop_iter();
                                if test.meter.update_from_iter(iter) {
                                    test.last_peak = Instant::now();
                                }

                                if test.last_rms.elapsed() > Duration::from_millis(150) {
                                    test.sender.try_send(Loudness {
                                        rms: dbfs(test.meter.rms()),
                                        peak: dbfs(test.meter.peak()),
                                    });

                                    test.last_rms = Instant::now();
                                }

                                if test.last_peak.elapsed() > Duration::from_millis(500) {
                                    test.meter.reset_peak();
                                    test.last_peak = Instant::now();
                                }
                            }
                        }
                    }

                    match client.deactivate() {
                        Ok(_) => {
                            let _ = sender.blocking_send(Event::Error(Error::ConnectionLost));
                            state = State::Error(0);
                        }
                        Err(err) => {
                            dbg!(err);
                            state = State::Error(0);
                        }
                    }
                }
                State::Error(retry_count) => {
                    const SLEEP_TIME_BASE: u64 = 3;

                    let timeout = retry_count * SLEEP_TIME_BASE;
                    let timeout = Duration::from_secs(timeout);

                    let _ = sender.blocking_send(Event::RetryIn(timeout));
                    std::thread::sleep(timeout);

                    state = State::NotConnected(retry_count + 1);
                }
            }
        }
    }

    fn start_jack_client(
        notify_sender: std::sync::mpsc::SyncSender<Notification>,
        has_server_shutdown: Arc<AtomicBool>,
    ) -> Result<
        (
            jack::AsyncClient<Notifications, ProcessHandler>,
            std::sync::mpsc::SyncSender<ProcessHandlerMessage>,
        ),
        Error,
    > {
        let client_name = env!("CARGO_BIN_NAME");
        let (client, _status) =
            jack::Client::new(client_name, jack::ClientOptions::NO_START_SERVER)?;

        // TODO: make configureable
        let out_port = client.register_port("measurement_out", jack::AudioOut::default())?;
        let in_port = client.register_port("measurement_in", jack::AudioIn::default())?;

        let out_port_name = out_port.name()?.to_string();
        let in_port_name = in_port.name()?.to_string();

        let notification_handler = Notifications::new(
            in_port_name,
            out_port_name,
            notify_sender,
            has_server_shutdown,
        );

        let (process_handler, process_sender) = ProcessHandler::new(out_port, in_port);
        let client = client.activate_async(notification_handler, process_handler)?;

        Ok((client, process_sender))
    }

    struct ProcessHandler {
        out_port: jack::Port<jack::AudioOut>,
        in_port: jack::Port<jack::AudioIn>,

        msg_receiver: std::sync::mpsc::Receiver<ProcessHandlerMessage>,

        state: ProcessHandlerState,
    }

    enum ProcessHandlerMessage {
        Stop,
        LoudnessMeasurement {
            in_buf: HeapCons<f32>,
            out_buf: HeapProd<f32>,
        },
    }

    enum ProcessHandlerState {
        Idle,
        LoudnessMeasurement {
            in_buf: HeapCons<f32>,
            out_buf: HeapProd<f32>,
        },
    }

    impl ProcessHandler {
        fn new(
            out_port: jack::Port<jack::AudioOut>,
            in_port: jack::Port<jack::AudioIn>,
        ) -> (Self, std::sync::mpsc::SyncSender<ProcessHandlerMessage>) {
            let (msg_sender, msg_receiver) = std::sync::mpsc::sync_channel(64);

            (
                Self {
                    out_port,
                    in_port,
                    msg_receiver,

                    state: ProcessHandlerState::Idle,
                },
                msg_sender,
            )
        }
    }

    impl jack::ProcessHandler for ProcessHandler {
        fn process(
            &mut self,
            _: &jack::Client,
            process_scope: &jack::ProcessScope,
        ) -> jack::Control {
            if let Ok(msg) = self.msg_receiver.try_recv() {
                match msg {
                    ProcessHandlerMessage::Stop => self.state = ProcessHandlerState::Idle,
                    ProcessHandlerMessage::LoudnessMeasurement { in_buf, out_buf } => {
                        self.state = ProcessHandlerState::LoudnessMeasurement { in_buf, out_buf }
                    }
                }
            }

            match &mut self.state {
                ProcessHandlerState::Idle => {
                    let out_port = self.out_port.as_mut_slice(process_scope);

                    out_port.fill_with(|| 0.0);
                }
                ProcessHandlerState::LoudnessMeasurement { in_buf, out_buf } => {
                    let out_port = self.out_port.as_mut_slice(process_scope);
                    in_buf.pop_slice(out_port);

                    let in_port = self.in_port.as_slice(process_scope);
                    out_buf.push_slice(in_port);
                }
            }
            jack::Control::Continue
        }
    }

    struct Notifications {
        in_port_name: String,
        out_port_name: String,
        notification_sender: std::sync::mpsc::SyncSender<Notification>,
        has_server_shutdown: Arc<AtomicBool>,
    }

    impl Notifications {
        pub fn new(
            in_port_name: String,
            out_port_name: String,
            notification_sender: std::sync::mpsc::SyncSender<Notification>,
            has_server_shutdown: Arc<AtomicBool>,
        ) -> Self {
            Self {
                in_port_name,
                out_port_name,
                notification_sender,
                has_server_shutdown,
            }
        }
    }

    impl jack::NotificationHandler for Notifications {
        fn thread_init(&self, _: &jack::Client) {}

        unsafe fn shutdown(&mut self, _: jack::ClientStatus, _: &str) {
            self.has_server_shutdown
                .store(true, std::sync::atomic::Ordering::Relaxed);
        }

        fn freewheel(&mut self, _: &jack::Client, _is_enabled: bool) {}

        fn sample_rate(&mut self, _: &jack::Client, _srate: jack::Frames) -> jack::Control {
            jack::Control::Continue
        }

        fn client_registration(&mut self, _: &jack::Client, _name: &str, _is_reg: bool) {}

        fn port_registration(&mut self, _: &jack::Client, _port_id: jack::PortId, _is_reg: bool) {}

        fn port_rename(
            &mut self,
            _: &jack::Client,
            _port_id: jack::PortId,
            _old_name: &str,
            _new_name: &str,
        ) -> jack::Control {
            jack::Control::Continue
        }

        fn ports_connected(
            &mut self,
            client: &jack::Client,
            port_id_a: jack::PortId,
            port_id_b: jack::PortId,
            are_connected: bool,
        ) {
            let Some(port_a) = client.port_by_id(port_id_a).and_then(|p| p.name().ok()) else {
                return;
            };
            let Some(port_b) = client.port_by_id(port_id_b).and_then(|p| p.name().ok()) else {
                return;
            };

            let out_port = &self.out_port_name;
            let dest_port = match (&port_a, &port_b) {
                (mine, port_b) if mine == out_port => Some(port_b),
                (port_a, mine) if mine == out_port => Some(port_a),
                _ => None,
            };

            let event = dest_port.cloned().map(|dest_port| {
                if are_connected {
                    Notification::OutPortConnected(dest_port)
                } else {
                    Notification::OutPortDisconnected(dest_port)
                }
            });

            if let Some(event) = event {
                let _ = self.notification_sender.send(event);
            }

            let in_port = &self.in_port_name;
            let dest_port = match (&port_a, &port_b) {
                (mine, port_b) if mine == in_port => Some(port_b),
                (port_a, mine) if mine == in_port => Some(port_a),
                _ => None,
            };

            let event = dest_port.cloned().map(|dest_port| {
                if are_connected {
                    Notification::InPortConnected(dest_port)
                } else {
                    Notification::InPortDisconnected(dest_port)
                }
            });

            if let Some(event) = event {
                dbg!(&event);
                let _ = self.notification_sender.send(event);
            }
        }

        fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
            jack::Control::Continue
        }

        fn xrun(&mut self, _: &jack::Client) -> jack::Control {
            jack::Control::Continue
        }
    }

    #[derive(Debug, Clone, thiserror::Error)]
    #[error("audio backend failed")]
    pub enum Error {
        #[error("jack audio server failed: {0}")]
        Jack(#[from] jack::Error),
        #[error("lost connection to jack audio server")]
        ConnectionLost,
    }
}
