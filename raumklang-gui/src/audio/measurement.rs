use crate::log;

use super::{loudness, Process, Stop};

use ringbuf::{
    consumer::PopIter,
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
    // signal_buf_underruns: AtomicUsize,
    // recording_buf_underruns: AtomicUsize,
    producer_dropped: AtomicBool,
    consumer_dropped: AtomicBool,
}

pub enum PopState<'a> {
    Some(PopIter<'a, HeapCons<f32>>),
    Exhausted(PopIter<'a, HeapCons<f32>>),
    ConsumerDropped,
}

impl Producer {
    pub fn pop_iter(&mut self) -> PopState {
        if self.state.consumer_dropped.load(atomic::Ordering::Acquire) {
            PopState::ConsumerDropped
        } else if self.state.signal_exhausted.load(atomic::Ordering::Acquire) {
            PopState::Exhausted(self.signal_cons.pop_iter())
        } else {
            PopState::Some(self.signal_cons.pop_iter())
        }
    }
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

            if processor.process(&data).is_err() {
                dbg!("processor errored, drop consumer");
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
    fn process(&mut self, data: &[f32]) -> Result<(), Stop> {
        self.loudness.process(data)?;

        if let Err(err) = self.data_sender.try_send(data.to_vec().into_boxed_slice()) {
            log::error!("failed to send measurement data to UI {err}");
        }

        Ok(())
    }
}
