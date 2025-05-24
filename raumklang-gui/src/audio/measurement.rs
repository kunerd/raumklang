use crate::log;

use super::{loudness, Process};

use ringbuf::{
    traits::{Consumer as _, Producer as _, Split as _},
    HeapCons, HeapProd, HeapRb,
};

use std::{
    sync::{atomic::AtomicBool, Arc},
    time::Duration,
};

pub fn create<'a, Signal>(buf_size: usize, signal: Signal) -> (Producer, Consumer<Signal>) {
    let (signal_prod, signal_cons) = HeapRb::new(buf_size).split();
    let (recording_prod, recording_cons) = HeapRb::new(buf_size).split();
    let stop = Arc::new(AtomicBool::new(false));

    let producer = Producer {
        in_buf: signal_cons,
        out_buf: recording_prod,
        stop: stop.clone(),
    };

    let consumer = Consumer {
        signal,
        signal_prod,
        recording_cons,
        stop,
    };

    (producer, consumer)
}

pub struct Producer {
    pub in_buf: HeapCons<f32>,
    pub out_buf: HeapProd<f32>,
    pub stop: Arc<AtomicBool>,
}

pub struct Consumer<Signal> {
    signal: Signal,
    signal_prod: HeapProd<f32>,
    recording_cons: HeapCons<f32>,
    stop: Arc<AtomicBool>,
}

impl<Signal> Consumer<Signal>
where
    Signal: IntoIterator<Item = f32>,
{
    pub fn run<P>(mut self, mut process: P)
    where
        P: Process,
    {
        let mut signal = self.signal.into_iter();

        loop {
            self.signal_prod.push_iter(&mut signal);

            let data: Vec<f32> = self.recording_cons.pop_iter().collect();
            process.process(&data);

            if self.stop.load(std::sync::atomic::Ordering::Acquire) == true {
                break;
            }

            // FIXME: calculate sleep duration from buf size and sample_rate
            std::thread::sleep(Duration::from_millis(10));
        }
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
    fn process(&mut self, data: &[f32]) {
        self.loudness.process(data);

        if let Err(err) = self.data_sender.try_send(data.to_vec().into_boxed_slice()) {
            log::error!("failed to send measurement data to UI {err}");
        }
    }
}
