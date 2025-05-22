use crate::log;

use super::Loudness;

use raumklang_core::{loudness, LinearSineSweep};
use ringbuf::{
    traits::{Consumer as _, Producer as _, Split as _},
    HeapCons, HeapProd, HeapRb,
};

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
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
    pub fn run<F>(mut self, mut process: F)
    where
        F: FnMut(Vec<f32>),
    {
        let mut signal = self.signal.into_iter();

        loop {
            self.signal_prod.push_iter(&mut signal);

            let data = self.recording_cons.pop_iter().collect();

            (process)(data);

            if self.stop.load(std::sync::atomic::Ordering::Acquire) == true {
                break;
            }
        }
    }
}

pub struct Measurement {
    last_rms: Instant,
    last_peak: Instant,
    sweep: LinearSineSweep,
    signal_prod: HeapProd<f32>,
    recording_cons: HeapCons<f32>,
    meter: loudness::Meter,
    loudness_sender: tokio::sync::mpsc::Sender<Loudness>,
    data_sender: tokio::sync::mpsc::Sender<Box<[f32]>>,
    stop: Arc<AtomicBool>,
}

impl Measurement {
    pub fn new(
        sweep: LinearSineSweep,
        loudness_sender: tokio::sync::mpsc::Sender<Loudness>,
        data_sender: tokio::sync::mpsc::Sender<Box<[f32]>>,
        signal_prod: HeapProd<f32>,
        recording_cons: HeapCons<f32>,
        stop: Arc<AtomicBool>,
    ) -> Self {
        let last_rms = Instant::now();
        let last_peak = Instant::now();

        // FIXME hardcoded sample rate dependency
        let meter = loudness::Meter::new(13230); // 44100samples / 1000ms * 300ms

        Self {
            last_rms,
            last_peak,
            meter,
            sweep,
            signal_prod,
            recording_cons,
            data_sender,
            loudness_sender,
            stop,
        }
    }

    pub fn run(mut self) {
        loop {
            self.signal_prod.push_iter(&mut self.sweep);

            let iter = self.recording_cons.pop_iter();
            let data: Vec<f32> = iter.collect();
            if self.meter.update_from_iter(data.iter().copied()) {
                self.last_peak = Instant::now();
            }

            if let Err(err) = self.data_sender.try_send(data.into_boxed_slice()) {
                log::error!("failed to send measurement data to UI {err}");
            }

            if self.last_rms.elapsed() > Duration::from_millis(150) {
                self.loudness_sender.try_send(Loudness {
                    rms: raumklang_core::dbfs(self.meter.rms()),
                    peak: raumklang_core::dbfs(self.meter.peak()),
                });

                self.last_rms = Instant::now();
            }

            if self.last_peak.elapsed() > Duration::from_millis(500) {
                self.meter.reset_peak();
                self.last_peak = Instant::now();
            }

            if self.stop.load(Ordering::Acquire) == true {
                break;
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
