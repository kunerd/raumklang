mod loudness;
mod measurement;

pub use loudness::Loudness;
use loudness::Test;
pub use measurement::Measurement;

use crate::data::{self};
use crate::log;

use iced::futures::Stream;
use jack::PortFlags;
use ringbuf::traits::{Consumer, Producer};
use tokio::sync::mpsc;
use tokio::sync::mpsc::error::TryRecvError;
use tokio_stream::wrappers::ReceiverStream;

use std::sync::atomic::{self, AtomicBool};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

#[derive(Debug, Clone)]
pub enum Event {
    Ready(Backend, Arc<mpsc::Receiver<Notification>>),
    Error(Error),
    RetryIn(Duration),
}

#[derive(Debug, Clone)]
pub enum Notification {
    OutPortConnected(String),
    OutPortDisconnected,
    InPortConnected(String),
    InPortDisconnected,
}

pub fn run() -> impl Stream<Item = Event> {
    let (sender, receiver) = mpsc::channel(1024);

    std::thread::spawn(|| run_audio_backend(sender));

    ReceiverStream::new(receiver)
}

#[derive(Debug, Clone)]
pub struct Backend {
    pub sample_rate: data::SampleRate,
    pub in_ports: Vec<String>,
    pub out_ports: Vec<String>,
    sender: mpsc::Sender<Command>,
}

pub trait Process {
    fn process(&mut self, data: &[f32]) -> Result<(), Stop>;
}

pub struct Stop;

impl Backend {
    pub fn run_test(&self, duration: Duration) -> mpsc::Receiver<Loudness> {
        let (loudness_sender, loudness_receiver) = mpsc::channel(128);

        let command = Command::RunTest {
            duration,
            loudness: loudness_sender,
        };

        self.sender.try_send(command).unwrap();

        loudness_receiver
    }

    pub fn run_measurement(
        &self,
        start_frequency: u16,
        end_frequency: u16,
        duration: Duration,
    ) -> (mpsc::Receiver<Loudness>, mpsc::Receiver<Box<[f32]>>) {
        let (loudness_sender, loudness_receiver) = mpsc::channel(128);
        let (data_sender, data_receiver) = mpsc::channel(1024);

        let command = Command::RunMeasurement {
            duration,
            start_frequency,
            end_frequency,
            data_sender,
            loudness_sender,
        };

        self.sender.try_send(command).unwrap();

        (loudness_receiver, data_receiver)
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

    pub async fn set_volume(self, volume: f32) {
        let command = Command::SetVolume(volume);

        let _ = self.sender.send(command).await;
    }
}

enum Command {
    RunTest {
        duration: Duration,
        loudness: mpsc::Sender<Loudness>,
    },
    ConnectOutPort(String),
    ConnectInPort(String),
    Stop,
    RunMeasurement {
        duration: Duration,
        loudness_sender: mpsc::Sender<Loudness>,
        data_sender: mpsc::Sender<Box<[f32]>>,
        start_frequency: u16,
        end_frequency: u16,
    },
    SetVolume(f32),
}

enum State {
    NotConnected(u64),
    Connected {
        client: jack::AsyncClient<Notifications, ProcessHandler>,
        commands: mpsc::Receiver<Command>,
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
                let (notification_sender, notification_receiver) = mpsc::channel(128);

                match start_jack_client(notification_sender, is_server_shutdown.clone()) {
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
                        let backend = Backend {
                            sample_rate,
                            in_ports,
                            out_ports,
                            sender: command_sender,
                        };
                        let _ = sender
                            .blocking_send(Event::Ready(backend, Arc::new(notification_receiver)));

                        state = State::Connected {
                            client,
                            commands: command_receiver,
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
                process,
                is_server_shutdown,
            } => {
                while is_server_shutdown.load(std::sync::atomic::Ordering::Relaxed) != true {
                    // FIXME: wrong channel type
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
                        Ok(Command::SetVolume(volume)) => {
                            let msg = ProcessHandlerMessage::SetAmplitude(
                                raumklang_core::volume_to_amplitude(volume),
                            );

                            process.send(msg);
                        }
                        Ok(Command::RunTest {
                            duration,
                            loudness: sender,
                        }) => {
                            let sample_rate = client.as_client().sample_rate();
                            let signal = raumklang_core::PinkNoise::with_amplitude(0.8)
                                .take_duration(
                                    sample_rate,
                                    data::Samples::from_duration(
                                        duration,
                                        data::SampleRate::new(sample_rate as u32),
                                    )
                                    .into(),
                                );

                            let buf_size = client.as_client().buffer_size() as usize;
                            let (producer, consumer) = measurement::create(buf_size);

                            let process_msg = ProcessHandlerMessage::Measurement(producer);
                            process.send(process_msg);

                            let test_process = Test::new(sender);
                            std::thread::spawn(move || {
                                consumer.run(signal, test_process);
                            });
                        }
                        Ok(Command::RunMeasurement {
                            start_frequency,
                            end_frequency,
                            duration,
                            loudness_sender,
                            data_sender,
                        }) => {
                            let sample_rate = client.as_client().sample_rate();
                            let sweep = raumklang_core::LinearSineSweep::new(
                                start_frequency,
                                end_frequency,
                                duration.as_secs() as usize,
                                0.8,
                                sample_rate,
                            );

                            let buf_size = client.as_client().buffer_size() as usize;
                            let (producer, consumer) = measurement::create(buf_size);

                            let process_msg = ProcessHandlerMessage::Measurement(producer);
                            process.send(process_msg);

                            let loudness = loudness::Test::new(loudness_sender);
                            let measurement = Measurement::new(loudness, data_sender);
                            std::thread::spawn(move || {
                                consumer.run(sweep, measurement);
                            });
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

                    thread::sleep(Duration::from_millis(100));
                }

                match client.deactivate() {
                    Ok(_) => {
                        let _ = sender.blocking_send(Event::Error(Error::ConnectionLost));
                        state = State::Error(0);
                    }
                    Err(err) => {
                        log::error!("{}", err);
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
    notify_sender: mpsc::Sender<Notification>,
    has_server_shutdown: Arc<AtomicBool>,
) -> Result<
    (
        jack::AsyncClient<Notifications, ProcessHandler>,
        std::sync::mpsc::SyncSender<ProcessHandlerMessage>,
    ),
    Error,
> {
    let client_name = env!("CARGO_BIN_NAME");
    let (client, _status) = jack::Client::new(client_name, jack::ClientOptions::NO_START_SERVER)?;

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
    amplitued: f32,

    msg_receiver: std::sync::mpsc::Receiver<ProcessHandlerMessage>,

    state: ProcessHandlerState,
}

enum ProcessHandlerMessage {
    Stop,
    SetAmplitude(f32),
    Measurement(measurement::Producer),
}

#[derive(Default)]
enum ProcessHandlerState {
    #[default]
    Idle,
    Measurement(measurement::Producer),
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
                amplitued: 0.5,

                msg_receiver,
                state: ProcessHandlerState::Idle,
            },
            msg_sender,
        )
    }
}

impl jack::ProcessHandler for ProcessHandler {
    fn process(&mut self, _: &jack::Client, process_scope: &jack::ProcessScope) -> jack::Control {
        if let Ok(msg) = self.msg_receiver.try_recv() {
            match msg {
                ProcessHandlerMessage::Stop => self.state = ProcessHandlerState::Idle,
                ProcessHandlerMessage::SetAmplitude(amplitude) => self.amplitued = amplitude,
                ProcessHandlerMessage::Measurement(producer) => {
                    self.state = ProcessHandlerState::Measurement(producer)
                }
            }
        }

        let state = std::mem::take(&mut self.state);
        self.state = match state {
            ProcessHandlerState::Idle => {
                let out_port = self.out_port.as_mut_slice(process_scope);

                out_port.fill_with(|| 0.0);

                ProcessHandlerState::Idle
            }
            ProcessHandlerState::Measurement(mut producer) => {
                {
                    if producer
                        .state
                        .consumer_dropped
                        .load(atomic::Ordering::Acquire)
                    {
                        // FIXME: hacky as fuck
                        dbg!("consumer dropped, go to idle");
                        self.state = ProcessHandlerState::Idle;
                        return jack::Control::Continue;
                    }

                    let out_port = self.out_port.as_mut_slice(process_scope);
                    let mut signal = producer.in_buf.pop_iter();
                    for o in out_port.iter_mut() {
                        if let Some(s) = signal.next() {
                            *o = s * self.amplitued;
                        } else {
                            *o = 0.0;
                            if producer
                                .state
                                .signal_exhausted
                                .load(atomic::Ordering::Acquire)
                            {
                                // FIXME: hacky as fuck
                                self.state = ProcessHandlerState::Idle;
                                return jack::Control::Continue;
                            }
                        }
                    }

                    let in_port = self.in_port.as_slice(process_scope);
                    producer.out_buf.push_slice(in_port);
                }

                ProcessHandlerState::Measurement(producer)
            }
        };

        jack::Control::Continue
    }
}

struct Notifications {
    in_port_name: String,
    out_port_name: String,
    notification_sender: mpsc::Sender<Notification>,
    has_server_shutdown: Arc<AtomicBool>,
}

impl Notifications {
    pub fn new(
        in_port_name: String,
        out_port_name: String,
        notification_sender: mpsc::Sender<Notification>,
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
                Notification::OutPortDisconnected
            }
        });

        if let Some(event) = event {
            let _ = self.notification_sender.blocking_send(event);
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
                Notification::InPortDisconnected
            }
        });

        if let Some(event) = event {
            let _ = self.notification_sender.blocking_send(event);
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
