use crate::log;

use super::Loudness;

use raumklang_core::{loudness, LinearSineSweep};
use ringbuf::{
    traits::{Consumer, Producer},
    HeapCons, HeapProd,
};

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::{Duration, Instant},
};

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
