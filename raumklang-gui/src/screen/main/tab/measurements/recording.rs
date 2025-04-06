use audio_backend::AudioBackend;
use iced::{
    widget::{button, column, container, pick_list, row, text, Button},
    Color, Element, Length, Subscription, Task,
};

use crate::{data, widgets::colored_circle};

pub struct Recording {
    audio_backend: BackendState,
    out_ports: Vec<String>,
    selected_out_port: Option<String>,
}

enum BackendState {
    NotConnected,
    Connected {
        sample_rate: data::SampleRate,
        backend: AudioBackend,
    },
    Retrying(std::time::Duration),
}

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    OutPortSelected(String),
    PlayPinkNoise,
    AudioBackend(audio_backend::Event),
    SampleRateChanged(data::SampleRate),
}

pub enum Action {
    None,
    Back,
    Task(Task<Message>),
}

impl Recording {
    pub fn new() -> Self {
        Self {
            audio_backend: BackendState::NotConnected,
            selected_out_port: None,
            out_ports: vec![],
        }
    }

    pub fn update(&mut self, message: Message) -> Action {
        match message {
            Message::Back => Action::Back,
            Message::OutPortSelected(port) => {
                self.selected_out_port = Some(port);
                Action::None
            }
            Message::PlayPinkNoise => Action::None,
            Message::AudioBackend(event) => match event {
                audio_backend::Event::Ready(backend, sample_rate) => {
                    self.audio_backend = BackendState::Connected {
                        backend: backend.clone(),
                        sample_rate,
                    };
                    Action::None
                }
                audio_backend::Event::Error(err) => {
                    println!("{err}");
                    Action::None
                }
                audio_backend::Event::RetryIn(duration) => {
                    self.audio_backend = BackendState::Retrying(duration);
                    Action::None
                }
            },
            Message::SampleRateChanged(new_sample_rate) => {
                let BackendState::Connected { sample_rate, .. } = &mut self.audio_backend else {
                    return Action::None;
                };

                *sample_rate = new_sample_rate;

                Action::None
            }
        }
    }

    pub fn view(&self) -> Element<'_, Message> {
        let content: Element<_> = {
            match &self.audio_backend {
                BackendState::NotConnected => container(text("Jack is not connected."))
                    .center(Length::Fill)
                    .into(),
                BackendState::Connected { sample_rate, .. } => {
                    let header = row![text!("Sample rate: {}", sample_rate)];

                    column![
                        header,
                        text("Recording").size(24),
                        row![
                            pick_list(
                                self.out_ports.as_slice(),
                                self.selected_out_port.as_ref(),
                                Message::OutPortSelected
                            ),
                            button("Play test signal").on_press(Message::PlayPinkNoise)
                        ],
                        column![button("Cancel").on_press(Message::Back)]
                    ]
                    .spacing(10)
                    .into()
                }
                BackendState::Retrying(duration) => container(column![
                    text("Something went wrong."),
                    text!("Retrying in: {}s", duration.as_secs())
                ])
                .center(Length::Fill)
                .into(),
            }
        };

        container(content).width(Length::Fill).into()
    }

    pub fn subscription(&self) -> Subscription<Message> {
        Subscription::run(audio_backend::run).map(Message::AudioBackend)
    }
}

pub fn recording_button<'a, Message: 'a>(msg: Message) -> Button<'a, Message> {
    button(colored_circle(8.0, Color::from_rgb8(200, 56, 42))).on_press(msg)
}

mod audio_backend {

    use std::time::Duration;

    // use iced::futures::channel::{mpsc};
    use iced::futures::sink::SinkExt;
    use iced::futures::{self, Stream, StreamExt};
    use iced::stream;
    use tokio::sync::mpsc;

    use crate::data::{self};

    #[derive(Debug, Clone)]
    pub enum Event {
        Ready(AudioBackend, data::SampleRate),
        Error(Error),
        RetryIn(Duration),
    }

    #[derive(Debug, Clone)]
    pub struct AudioBackend(mpsc::Sender<ActorMessage>);

    impl AudioBackend {
        fn new(
            notification_sender: mpsc::Sender<JackNotification>,
        ) -> Result<(Self, BackendActor), Error> {
            let (sender, receiver) = mpsc::channel(42);

            let actor = BackendActor::new(notification_sender, receiver)?;

            Ok((Self(sender), actor))
        }

        // pub async fn sample_rate(mut self) -> data::SampleRate {
        //     let (sender, receiver) = oneshot::channel();

        //     let _ = self.0.send(ActorMessage::GetSampleRate(sender)).await;

        //     receiver.await.unwrap()
        // }
    }

    struct BackendActor {
        receiver: mpsc::Receiver<ActorMessage>,
        client: jack::AsyncClient<Notifications, ProcessHandler>,
    }

    enum ActorMessage {
        // GetSampleRate(channel::oneshot::Sender<data::SampleRate>),
    }

    impl BackendActor {
        fn new(
            sender: mpsc::Sender<JackNotification>,
            receiver: mpsc::Receiver<ActorMessage>,
        ) -> Result<Self, Error> {
            let jack_client_name = env!("CARGO_BIN_NAME");

            // TODO check status
            let (client, _status) =
                jack::Client::new(&jack_client_name, jack::ClientOptions::NO_START_SERVER)?;

            let handler = ProcessHandler {};
            let client = client.activate_async(Notifications(sender), handler)?;

            Ok(Self { receiver, client })
        }

        async fn sample_rate(&self) -> data::SampleRate {
            self.client.as_client().sample_rate().into()
        }

        async fn handle_message(&mut self, message: ActorMessage) {
            match message {
                // ActorMessage::GetSampleRate(sender) => {
                //     let _ = sender.send(self.sample_rate().await);
                // }
            }
        }
    }

    enum State {
        NotConnected(u64),
        Connected(mpsc::Receiver<JackNotification>, BackendActor),
        Error(u64),
    }

    pub fn run() -> impl Stream<Item = Event> {
        stream::channel(100, async |mut output| {
            let mut state = State::NotConnected(0);

            loop {
                match &mut state {
                    State::NotConnected(retry_count) => {
                        let (notification_sender, notification_receiver) = mpsc::channel(64);
                        match AudioBackend::new(notification_sender) {
                            Ok((backend, actor)) => {
                                let sample_rate = actor.sample_rate().await;
                                let _ = output.send(Event::Ready(backend, sample_rate)).await;
                                state = State::Connected(notification_receiver, actor)
                            }
                            Err(err) => {
                                let _ = output.send(Event::Error(err)).await;
                                state = State::Error(*retry_count);
                            }
                        }
                    }
                    State::Connected(notification_receiver, actor) => {
                        tokio::select! {
                            Some(message) = actor.receiver.recv() => {
                                actor.handle_message(message).await;
                            },
                            maybe_notification = notification_receiver.recv() => {
                                match maybe_notification {
                                    Some(JackNotification::Shutdown) => {
                                        state = State::Error(0);
                                    }
                                    Some(JackNotification::SampleRateChanged(srate)) => {
                                        println!("srate: {srate}");
                                    }
                                    None => state = State::Error(0)
                                }
                                println!("Something went wrong");
                            }
                            else => {
                                println!("Something went wrong");
                            }
                        }
                        // if let Some(message) = actor.receiver.next().await {
                        //     actor.handle_message(message).await;
                        // }
                    }
                    State::Error(retry_count) => {
                        const SLEEP_TIME_BASE: u64 = 3;

                        let timeout = *retry_count * SLEEP_TIME_BASE;
                        let timeout = Duration::from_secs(timeout);

                        let _ = output.send(Event::RetryIn(timeout)).await;
                        tokio::time::sleep(timeout).await;

                        state = State::NotConnected(*retry_count + 1);
                    }
                }
            }
        })
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

    enum JackNotification {
        Shutdown,
        SampleRateChanged(data::SampleRate),
    }

    struct Notifications(mpsc::Sender<JackNotification>);

    impl jack::NotificationHandler for Notifications {
        fn thread_init(&self, _: &jack::Client) {
            println!("JACK: thread init");
        }

        /// Not much we can do here, see https://man7.org/linux/man-pages/man7/signal-safety.7.html.
        unsafe fn shutdown(&mut self, _: jack::ClientStatus, _: &str) {
            println!("JACK: shutdown");
            // let _ = self.0.blocking_send(JackNotification::Shutdown);
            let _ = self.0.try_send(JackNotification::Shutdown);
        }

        fn freewheel(&mut self, _: &jack::Client, is_enabled: bool) {
            println!(
                "JACK: freewheel mode is {}",
                if is_enabled { "on" } else { "off" }
            );
        }

        fn sample_rate(&mut self, _: &jack::Client, srate: jack::Frames) -> jack::Control {
            println!("JACK: sample rate changed to {srate}");
            let _ = self
                .0
                .try_send(JackNotification::SampleRateChanged(srate.into()));
            jack::Control::Continue
        }

        fn client_registration(&mut self, _: &jack::Client, name: &str, is_reg: bool) {
            println!(
                "JACK: {} client with name \"{}\"",
                if is_reg { "registered" } else { "unregistered" },
                name
            );
        }

        fn port_registration(&mut self, _: &jack::Client, port_id: jack::PortId, is_reg: bool) {
            println!(
                "JACK: {} port with id {}",
                if is_reg { "registered" } else { "unregistered" },
                port_id
            );
        }

        fn port_rename(
            &mut self,
            _: &jack::Client,
            port_id: jack::PortId,
            old_name: &str,
            new_name: &str,
        ) -> jack::Control {
            println!("JACK: port with id {port_id} renamed from {old_name} to {new_name}",);
            jack::Control::Continue
        }

        fn ports_connected(
            &mut self,
            _: &jack::Client,
            port_id_a: jack::PortId,
            port_id_b: jack::PortId,
            are_connected: bool,
        ) {
            println!(
                "JACK: ports with id {} and {} are {}",
                port_id_a,
                port_id_b,
                if are_connected {
                    "connected"
                } else {
                    "disconnected"
                }
            );
        }

        fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
            println!("JACK: graph reordered");
            jack::Control::Continue
        }

        fn xrun(&mut self, _: &jack::Client) -> jack::Control {
            println!("JACK: xrun occurred");
            jack::Control::Continue
        }
    }

    #[derive(Debug, Clone, thiserror::Error)]
    pub enum Error {
        #[error("jack audio server failed: {0}")]
        Jack(#[from] jack::Error),
    }
}
