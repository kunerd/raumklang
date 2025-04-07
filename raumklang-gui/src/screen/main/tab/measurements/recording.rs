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
        // backend: AudioBackend,
    },
    Retrying(std::time::Duration),
    Error(audio_backend::Error),
}

#[derive(Debug, Clone)]
pub enum Message {
    Back,
    OutPortSelected(String),
    PlayPinkNoise,
    AudioBackend(audio_backend::Event),
    // SampleRateChanged(data::SampleRate),
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
                audio_backend::Event::Ready(sample_rate) => {
                    self.audio_backend = BackendState::Connected {
                        // backend: backend.clone(),
                        sample_rate,
                    };
                    Action::None
                }
                audio_backend::Event::Error(err) => {
                    println!("{err}");
                    self.audio_backend = BackendState::Error(err);
                    Action::None
                }
                audio_backend::Event::RetryIn(duration) => {
                    self.audio_backend = BackendState::Retrying(duration);
                    Action::None
                }
            },
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
                BackendState::Error(error) => container(column![
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
        Subscription::run(audio_backend::run).map(Message::AudioBackend)
    }
}

pub fn recording_button<'a, Message: 'a>(msg: Message) -> Button<'a, Message> {
    button(colored_circle(8.0, Color::from_rgb8(200, 56, 42))).on_press(msg)
}

mod audio_backend {

    use std::sync::atomic::AtomicBool;
    use std::sync::Arc;
    use std::time::Duration;

    use iced::futures::{SinkExt, Stream};
    use iced::stream;
    use tokio::sync::mpsc;

    use crate::data::{self};

    #[derive(Debug, Clone)]
    pub enum Event {
        Ready(data::SampleRate),
        Error(Error),
        RetryIn(Duration),
    }

    enum State {
        NotConnected(u64),
        Connected(
            jack::AsyncClient<Notifications, ProcessHandler>,
            Arc<AtomicBool>,
        ),
        Error(u64),
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

    fn run_audio_backend(sender: mpsc::Sender<Event>) {
        let mut state = State::NotConnected(0);

        loop {
            match state {
                State::NotConnected(retry_count) => {
                    let is_server_shutdown = Arc::new(AtomicBool::new(false));

                    match start_jack_client(is_server_shutdown.clone()) {
                        Ok(client) => {
                            let sample_rate = client.as_client().sample_rate().into();
                            let _ = sender.blocking_send(Event::Ready(sample_rate));

                            state = State::Connected(client, is_server_shutdown);
                        }
                        Err(err) => {
                            let _ = sender.blocking_send(Event::Error(err));
                            state = State::Error(retry_count);
                        }
                    }
                }
                State::Connected(client, is_server_shutdown) => {
                    while is_server_shutdown.load(std::sync::atomic::Ordering::Relaxed) != true {
                        std::thread::sleep(Duration::from_millis(50));
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
        // close: Arc<signal_hook::low_level::channel::Channel<bool>>,
        close: Arc<AtomicBool>,
    ) -> Result<jack::AsyncClient<Notifications, ProcessHandler>, Error> {
        println!("start jack client");

        let (client, _status) =
            jack::Client::new("threading_test", jack::ClientOptions::default())?;

        println!("start jack client, register port");
        client.register_port("rust_in_l", jack::AudioIn::default())?;

        println!("start jack client, activate async");
        Ok(client.activate_async(Notifications(close), ProcessHandler {})?)
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

    struct Notifications(Arc<AtomicBool>);

    impl jack::NotificationHandler for Notifications {
        fn thread_init(&self, _: &jack::Client) {}

        /// Not much we can do here, see https://man7.org/linux/man-pages/man7/signal-safety.7.html.
        unsafe fn shutdown(&mut self, _: jack::ClientStatus, _: &str) {
            self.0.store(true, std::sync::atomic::Ordering::Relaxed);
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
            _: &jack::Client,
            _port_id_a: jack::PortId,
            _port_id_b: jack::PortId,
            _are_connected: bool,
        ) {
        }

        fn graph_reorder(&mut self, _: &jack::Client) -> jack::Control {
            jack::Control::Continue
        }

        fn xrun(&mut self, _: &jack::Client) -> jack::Control {
            jack::Control::Continue
        }
    }

    #[derive(Debug, Clone, thiserror::Error)]
    pub enum Error {
        #[error("jack audio server failed: {0}")]
        Jack(#[from] jack::Error),
        #[error("lost connection to jack audio server")]
        ConnectionLost,
    }
}
