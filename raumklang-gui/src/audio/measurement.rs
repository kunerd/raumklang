use crate::log;

use super::{loudness, process::Control, Process};

use ringbuf::{
    traits::{Consumer as _, Producer as _, Split as _},
    HeapCons, HeapProd, HeapRb,
};

use std::{
    sync::{
        atomic::{self, AtomicBool},
        Arc,
    },
    time::Duration,
};

pub fn create(buf_size: usize) -> (Producer, Consumer) {
    let (signal_prod, signal_cons) = HeapRb::new(buf_size).split();
    let (recording_prod, recording_cons) = HeapRb::new(buf_size).split();

    let state = State {
        signal_exhausted: AtomicBool::new(false),
        producer_dropped: AtomicBool::new(false),
        consumer_dropped: AtomicBool::new(false),
    };
    let state = Arc::new(state);

    let producer = Producer {
        signal_cons,
        recording_prod,
        state: Arc::clone(&state),
    };

    let consumer = Consumer {
        signal_prod,
        recording_cons,
        state,
    };

    (producer, consumer)
}

pub struct Producer {
    signal_cons: HeapCons<f32>,
    pub recording_prod: HeapProd<f32>,
    state: Arc<State>,
}

pub struct Consumer {
    signal_prod: HeapProd<f32>,
    recording_cons: HeapCons<f32>,
    state: Arc<State>,
}

pub struct State {
    signal_exhausted: AtomicBool,
    producer_dropped: AtomicBool,
    consumer_dropped: AtomicBool,
}

impl Producer {
    #[must_use]
    pub fn play_signal_chunk<'a>(
        &'a mut self,
        out_port: &mut [f32],
        amplitude: f32,
    ) -> Option<SignalState> {
        let mut write_signal = || {
            let mut signal = self.signal_cons.pop_iter();
            let mut buf_empty = false;
            for o in out_port.iter_mut() {
                if let Some(s) = signal.next() {
                    *o = s * amplitude;
                } else {
                    *o = 0.0;
                    buf_empty = true;
                }
            }

            buf_empty
        };

        if self.state.consumer_dropped.load(atomic::Ordering::Acquire) {
            out_port.fill(0.0);
            None
        } else if self.state.signal_exhausted.load(atomic::Ordering::Acquire) {
            let buf_empty = write_signal();

            if buf_empty {
                Some(SignalState::FullyConsumed)
            } else {
                Some(SignalState::Exhausted)
            }
        } else {
            write_signal();
            Some(SignalState::NotExhausted)
        }
    }

    #[must_use]
    pub fn record_chunk(&mut self, chunk: &[f32]) -> Result<(), Error> {
        if self.state.consumer_dropped.load(atomic::Ordering::Acquire) {
            return Err(Error::ConsumerDropped);
        }

        self.recording_prod.push_slice(chunk);

        Ok(())
    }
}

pub enum Error {
    ConsumerDropped,
}

pub enum SignalState {
    NotExhausted,
    Exhausted,
    FullyConsumed,
}

impl Drop for Producer {
    fn drop(&mut self) {
        self.state
            .producer_dropped
            .store(true, atomic::Ordering::Release);
    }
}

impl Consumer {
    pub fn run<S, P>(mut self, signal: S, mut processor: P)
    where
        S: IntoIterator<Item = f32>,
        P: Process,
    {
        let mut signal = signal.into_iter().peekable();

        loop {
            self.signal_prod.push_iter(&mut signal);
            if signal.peek().is_none() {
                self.state
                    .signal_exhausted
                    .store(true, atomic::Ordering::Release);
            }

            let data: Vec<f32> = self.recording_cons.pop_iter().collect();
            if self
                .state
                .producer_dropped
                .load(std::sync::atomic::Ordering::Acquire)
                == true
                && data.is_empty()
            {
                break;
            }

            if let Control::Stop = processor.process(&data) {
                break;
            }

            // FIXME: calculate sleep duration from buf size and sample_rate
            std::thread::sleep(Duration::from_millis(10));
        }
    }
}

impl Drop for Consumer {
    fn drop(&mut self) {
        self.state
            .consumer_dropped
            .store(true, atomic::Ordering::Release);
    }
}

pub struct Measurement {
    loudness: loudness::Test,
    data_sender: tokio::sync::mpsc::Sender<Box<[f32]>>,
}

impl Measurement {
    pub fn new(
        loudness: loudness::Test,
        data_sender: tokio::sync::mpsc::Sender<Box<[f32]>>,
    ) -> Self {
        Self {
            loudness,
            data_sender,
        }
    }
}

impl Process for Measurement {
    fn process(&mut self, data: &[f32]) -> Control {
        if let Control::Stop = self.loudness.process(data) {
            return Control::Stop;
        }

        if let Err(err) = self.data_sender.try_send(data.to_vec().into_boxed_slice()) {
            log::error!("failed to send measurement data to UI {err}");
        }

        Control::Continue
    }
}
