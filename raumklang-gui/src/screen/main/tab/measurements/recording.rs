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
    Testing(mpsc::Sender<f32>), //     Testing,
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
            volume: 0.8,
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

                let Some(out_port) = &self.selected_out_port else {
                    return Action::None;
                };

                let Some(in_port) = &self.selected_in_port else {
                    return Action::None;
                };

                let duration = Duration::from_secs(3);
                let (_rms_receiver, volume_sender) = backend.run_test(out_port, in_port, duration);

                *measurement = MeasurementState::Testing(volume_sender);

                Action::None
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
            Message::OutPortSelected(port) => {
                self.selected_out_port = Some(port);
                self.check_port_state();

                Action::None
            }
            Message::InPortSelected(port) => {
                self.selected_in_port = Some(port);
                self.check_port_state();

                Action::None
            }
            Message::RetryTick(instant) => {
                let State::Retrying { end, remaining, .. } = &mut self.state else {
                    return Action::None;
                };

                *remaining = *end - instant;

                Action::None
            }
            Message::VolumeChanged(volume) => {
                self.volume = volume;

                Action::None
            }
            Message::StopTesting => {
                let State::Connected {
                    backend: _,
                    measurement: measurement @ MeasurementState::Testing(_),
                } = &mut self.state
                else {
                    return Action::None;
                };

                *measurement = MeasurementState::ReadyForTest;

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
                        MeasurementState::Testing(_sender) => container(column![
                            text("Test running ..."),
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
            MeasurementState::Testing(_sender) => {}
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
    use std::time::Duration;

    use iced::futures::{SinkExt, Stream};
    use iced::stream;
    use jack::PortFlags;
    use tokio::sync::mpsc;

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

    impl AudioBackend {
        pub fn run_test(
            &self,
            out_port: &str,
            in_port: &str,
            duration: Duration,
        ) -> (mpsc::Receiver<f32>, mpsc::Sender<f32>) {
            let (rms_sender, rms_receiver) = mpsc::channel(100);
            let (volume_sender, volume_receiver) = mpsc::channel(100);

            let command = Command::RunTest {
                out_port: out_port.to_string(),
                in_port: in_port.to_string(),
                duration,
                rms: rms_sender,
                volume: volume_receiver,
            };

            self.sender.try_send(command);

            (rms_receiver, volume_sender)
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
            out_port: String,
            in_port: String,
            duration: Duration,
            rms: mpsc::Sender<f32>,
            volume: mpsc::Receiver<f32>,
        },
    }

    enum State {
        NotConnected(u64),
        Connected {
            client: jack::AsyncClient<Notifications, ProcessHandler>,
            commands: mpsc::Receiver<Command>,
            events: std::sync::mpsc::Receiver<Notification>,
            is_server_shutdown: Arc<AtomicBool>,
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
                        Ok(client) => {
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
                    is_server_shutdown,
                } => {
                    while is_server_shutdown.load(std::sync::atomic::Ordering::Relaxed) != true {
                        match events.recv_timeout(Duration::from_millis(50)) {
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

                        match commands.blocking_recv() {
                            Some(_) => todo!(),
                            None => {
                                // their is no receiver anymore
                                return;
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
    ) -> Result<jack::AsyncClient<Notifications, ProcessHandler>, Error> {
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

        Ok(client.activate_async(notification_handler, ProcessHandler {})?)
    }

    pub struct ProcessHandler {
        // respond_to: Option<SyncSender<bool>>,
        // cur_signal: Option<I>,
        // out_port: Option<jack::Port<jack::AudioOut>>,
        // input: Option<(jack::Port<jack::AudioIn>, HeapProducer<f32>)>,
        // msg_rx: Receiver<Message<I, J>>,
    }

    impl jack::ProcessHandler for ProcessHandler {
        fn process(
            &mut self,
            _: &jack::Client,
            _process_scope: &jack::ProcessScope,
        ) -> jack::Control {
            // let mut signal_ended = false;

            // if let (Some(out), Some(signal)) = (&mut self.out_port, &mut self.cur_signal) {
            //     let out = out.as_mut_slice(process_scope);

            //     for o in out.iter_mut() {
            //         if let Some(sample) = signal.next() {
            //             *o = sample;
            //         } else {
            //             *o = 0.0f32;
            //             signal_ended = true;
            //         }
            //     }
            // };

            // if let Some((port, buf)) = &mut self.input {
            //     let in_a_p = port.as_slice(process_scope);
            //     buf.push_slice(in_a_p);
            // }

            // if signal_ended {
            //     let _ = self.respond_to.as_ref().unwrap().try_send(true);
            //     self.respond_to = None;
            //     self.cur_signal = None;
            // }

            // if let Ok(msg) = self.msg_rx.try_recv() {
            //     match msg {
            //         Message::RegisterOutPort(p) => self.out_port = Some(p),
            //         Message::RegisterInPort(port, prod) => self.input = Some((port, prod)),
            //         Message::PlaySignal { signal, respond_to } => {
            //             self.respond_to = Some(respond_to);
            //             self.cur_signal = Some(signal.into_iter());
            //         }
            //     }
            // }

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

            let ports = [port_a, port_b];
            let (src, dest): (Vec<_>, Vec<_>) =
                ports.iter().partition(|p| **p == self.out_port_name);

            if let (Some(_src), Some(dest)) = (src.first(), dest.first()) {
                let notification = match are_connected {
                    true => Notification::OutPortConnected(dest.to_string()),
                    false => Notification::OutPortDisconnected(dest.to_string()),
                };

                let _ = self.notification_sender.send(notification);
            }

            let (src, dest): (Vec<_>, Vec<_>) =
                ports.into_iter().partition(|p| p == &self.in_port_name);

            if let (Some(_src), Some(dest)) = (src.first(), dest.first()) {
                let notification = match are_connected {
                    true => Notification::InPortConnected(dest.to_string()),
                    false => Notification::InPortDisconnected(dest.to_string()),
                };

                let _ = self.notification_sender.send(notification);
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
