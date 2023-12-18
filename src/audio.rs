use std::sync::mpsc::{sync_channel, Receiver, SyncSender};

use ringbuf::{HeapConsumer, HeapProducer, HeapRb};

struct Notifications;

impl jack::NotificationHandler for Notifications {}

enum Message<I, J>
where
    I: Iterator<Item = f32>,
    J: IntoIterator<IntoIter = I>,
{
    RegisterOutPort(jack::Port<jack::AudioOut>),
    RegisterInPort(jack::Port<jack::AudioIn>, HeapProducer<f32>),
    PlaySignal{
        signal: J,
        respond_to: SyncSender<bool>
    },
}

pub struct ProcessHandler<I, J>
where
    I: Iterator<Item = f32>,
    J: IntoIterator<IntoIter = I>,
{
    respond_to: Option<SyncSender<bool>>,
    cur_signal: Option<I>,
    out_port: Option<jack::Port<jack::AudioOut>>,
    input: Option<(jack::Port<jack::AudioIn>, HeapProducer<f32>)>,
    msg_rx: Receiver<Message<I, J>>,
}

impl<I, J> jack::ProcessHandler for ProcessHandler<I, J>
where
    I: Iterator<Item = f32> + Send,
    J: IntoIterator<IntoIter = I> + Send,
{
    fn process(&mut self, _: &jack::Client, process_scope: &jack::ProcessScope) -> jack::Control {
        let mut signal_ended = false;

        if let Some(out) = &mut self.out_port {
            let out = out.as_mut_slice(process_scope);

            if let Some(iter) = &mut self.cur_signal {
                for o in out.iter_mut() {
                    if let Some(sample) = iter.next() {
                        *o = sample;
                    } else {
                        *o = 0.0f32;
                        signal_ended = true;
                    }
                }
            }
        };

        if let Some((port, buf)) = &mut self.input {
            let in_a_p = port.as_slice(process_scope);
            buf.push_slice(in_a_p);
        }

        if signal_ended {
            let _ = self.respond_to.as_ref().unwrap().send(true);
            self.respond_to = None;
            self.cur_signal = None;
        }

        if let Ok(msg) = self.msg_rx.try_recv() {
            match msg {
                Message::RegisterOutPort(p) => self.out_port = Some(p),
                Message::RegisterInPort(port, prod) => self.input = Some((port, prod)),
                Message::PlaySignal{ signal, respond_to } => {
                    self.respond_to = Some(respond_to);
                    self.cur_signal = Some(signal.into_iter());
                }
            }
        }

        jack::Control::Continue
    }
}

pub struct AudioEngine<I, J>
where
    I: Iterator<Item = f32>,
    J: IntoIterator<IntoIter = I>,
{
    client: jack::AsyncClient<(), ProcessHandler<I, J>>,
    msg_tx: SyncSender<Message<I, J>>,
}

impl<I, J> AudioEngine<I, J>
where
    I: Iterator<Item = f32> + Send + 'static,
    J: IntoIterator<IntoIter = I> + Send + Sync + 'static,
{
    pub fn new(name: &str) -> anyhow::Result<Self> {
        let (client, _status) = jack::Client::new(name, jack::ClientOptions::NO_START_SERVER)?;

        let (msg_tx, msg_rx) = sync_channel(64);

        let process_handler = ProcessHandler {
            respond_to: None,
            out_port: None,
            input: None,
            cur_signal: None,
            msg_rx,
        };

        let active_client = client.activate_async((), process_handler)?;

        Ok(Self {
            client: active_client,
            msg_tx,
        })
    }

    pub fn register_out_port<T: AsRef<str>>(
        &self,
        port_name: &str,
        dest_ports: &[T],
    ) -> anyhow::Result<()> {
        let out_port = self
            .client
            .as_client()
            .register_port(port_name, jack::AudioOut)?;

        let full_port_name = out_port.name()?;

        for dest_port in dest_ports {
            self.client
                .as_client()
                .connect_ports_by_name(&full_port_name, dest_port.as_ref())?;
        }

        self.msg_tx.send(Message::RegisterOutPort(out_port))?;

        Ok(())
    }

    pub fn register_in_port(
        &self,
        port_name: &str,
        input_port_name: &str,
    ) -> anyhow::Result<HeapConsumer<f32>> {
        const BUFF_SIZE: usize = 1024;

        let in_port = self
            .client
            .as_client()
            .register_port(port_name, jack::AudioIn)?;

        let rb = HeapRb::<_>::new(BUFF_SIZE);
        let (prod, cons) = rb.split();

        let full_port_name = in_port.name()?;
        self.client
            .as_client()
            .connect_ports_by_name(input_port_name, &full_port_name)?;

        self.msg_tx.send(Message::RegisterInPort(in_port, prod))?;

        Ok(cons)
    }

    pub fn sample_rate(&self) -> usize {
        self.client.as_client().sample_rate()
    }

    pub fn play_signal(&self, signal: J) -> anyhow::Result<Receiver<bool>> {
        let (tx, rx) = sync_channel(1);
        self.msg_tx.send(Message::PlaySignal {signal, respond_to: tx})?;

        Ok(rx)
    }
}
