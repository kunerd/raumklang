use std::time::Duration;

use iced::{
    alignment::Vertical,
    time,
    widget::{
        button, column, container, horizontal_rule, horizontal_space, pick_list, row, text, Button,
    },
    Color, Element, Length, Subscription, Task,
};

use crate::{data, widgets::colored_circle};

pub struct Recording {
    state: State,
    selected_out_port: Option<String>,
    selected_in_port: Option<String>,
}

enum State {
    NotConnected,
    Connected {
        sample_rate: data::SampleRate,
        out_ports: Vec<String>,
        in_ports: Vec<String>,
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
    //     Testing,
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
            selected_out_port: None,
            selected_in_port: None,
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Back => Action::Back,
            Message::RunTest => Action::None,
            Message::AudioBackend(event) => match event {
                audio_backend::Event::Ready {
                    sample_rate,
                    in_ports,
                    out_ports,
                } => {
                    self.state = State::Connected {
                        sample_rate,
                        in_ports,
                        out_ports,
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
                audio_backend::Event::Heartbeat => Action::None,
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
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<_> = {
            match &self.state {
                State::NotConnected => container(text("Jack is not connected."))
                    .center(Length::Fill)
                    .into(),
                State::Connected {
                    sample_rate,
                    out_ports,
                    in_ports,
                    measurement,
                } => {
                    let header = column![
                        row![
                            text("Recording").size(24),
                            horizontal_space(),
                            text!("Sample rate: {}", sample_rate).size(14)
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
                                            out_ports.as_slice(),
                                            self.selected_out_port.as_ref(),
                                            Message::OutPortSelected
                                        )
                                    ]
                                    .spacing(6),
                                    column![
                                        text("In port"),
                                        pick_list(
                                            in_ports.as_slice(),
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
                                            out_ports.as_slice(),
                                            self.selected_out_port.as_ref(),
                                            Message::OutPortSelected
                                        )
                                    ]
                                    .spacing(6),
                                    column![
                                        text("In port"),
                                        pick_list(
                                            in_ports.as_slice(),
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
        Ready {
            sample_rate: data::SampleRate,
            in_ports: Vec<String>,
            out_ports: Vec<String>,
        },
        Notification(Notification),
        Error(Error),
        RetryIn(Duration),
        Heartbeat,
    }

    #[derive(Debug, Clone)]
    pub enum Notification {
        OutPortConnected(String),
        OutPortDisconnected(String),
        InPortConnected(String),
        InPortDisconnected(String),
    }

    enum State {
        NotConnected(u64),
        Connected(
            jack::AsyncClient<Notifications, ProcessHandler>,
            std::sync::mpsc::Receiver<Notification>,
            Arc<AtomicBool>,
        ),
        Error(u64),
    }

    pub fn run() -> impl Stream<Item = Event> {
        stream::channel(100, async |mut output| {
            let (sender, mut receiver) = mpsc::channel(100);

            std::thread::spawn(|| run_audio_backend(sender));

            while let Some(event) = receiver.recv().await {
                if let Event::Heartbeat = event {
                    continue;
                }

                let _ = output.send(event).await;
            }
        })
    }

    fn run_audio_backend(sender: mpsc::Sender<Event>) {
        let mut state = State::NotConnected(0);

        loop {
            match state {
                State::NotConnected(retry_count) => {
                    let is_server_shutdown = Arc::new(AtomicBool::new(false));
                    let (notify_sender, notify_receiver) = std::sync::mpsc::sync_channel(64);

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

                            let _ = sender.blocking_send(Event::Ready {
                                sample_rate,
                                in_ports,
                                out_ports,
                            });

                            state = State::Connected(client, notify_receiver, is_server_shutdown);
                        }
                        Err(err) => {
                            let _ = sender.blocking_send(Event::Error(err));
                            state = State::Error(retry_count);
                        }
                    }
                }
                State::Connected(client, notify_receiver, is_server_shutdown) => {
                    while is_server_shutdown.load(std::sync::atomic::Ordering::Relaxed) != true {
                        match notify_receiver.recv_timeout(Duration::from_millis(50)) {
                            Ok(notificytion) => {
                                let _ = sender.blocking_send(Event::Notification(notificytion));
                            }
                            Err(RecvTimeoutError::Disconnected) => {
                                break;
                            }
                            Err(RecvTimeoutError::Timeout) => {
                                if sender.blocking_send(Event::Heartbeat).is_err() {
                                    // their is no receiver anymore
                                    return;
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
