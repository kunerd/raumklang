use raumklang_core::{dbfs, loudness};

use ringbuf::{
    traits::{Consumer, Producer},
    HeapCons, HeapProd,
};
use tokio::sync::mpsc::error::TrySendError;

use std::{
    sync::{atomic::AtomicBool, Arc},
    time::{Duration, Instant},
};

use crate::data;

#[derive(Debug, Clone, Copy)]
pub struct Loudness {
    pub rms: f32,
    pub peak: f32,
}

impl Default for Loudness {
    fn default() -> Self {
        Self {
            rms: f32::NEG_INFINITY,
            peak: f32::NEG_INFINITY,
        }
    }
}

pub struct Test {
    duration: Duration,
    last_rms: Instant,
    last_peak: Instant,
    meter: loudness::Meter,
    signal_prod: HeapProd<f32>,
    recording: HeapCons<f32>,
    sender: tokio::sync::mpsc::Sender<Loudness>,
    stop: Arc<AtomicBool>,
}

impl Test {
    pub fn new(
        duration: Duration,
        sender: tokio::sync::mpsc::Sender<Loudness>,
        stop: Arc<AtomicBool>,
        signal_prod: HeapProd<f32>,
        recording_cons: HeapCons<f32>,
    ) -> Self {
        let last_rms = Instant::now();
        let last_peak = Instant::now();

        // FIXME hardcoded sample rate dependency
        let meter = loudness::Meter::new(13230); // 44100samples / 1000ms * 300ms

        Self {
            duration,
            last_rms,
            last_peak,
            meter,
            signal_prod,
            recording: recording_cons,
            sender,
            stop,
        }
    }

    pub fn run(mut self) {
        // FIXME remove hard-coded values
        let mut signal = raumklang_core::PinkNoise::with_amplitude(0.8).take_duration(
            44100,
            data::Samples::from_duration(self.duration, data::SampleRate::new(44_100)).into(),
        );

        loop {
            self.signal_prod.push_iter(&mut signal);

            let iter = self.recording.pop_iter();
            if self.meter.update_from_iter(iter) {
                self.last_peak = Instant::now();
            }

            if self.last_rms.elapsed() > Duration::from_millis(150) {
                let loudness = Loudness {
                    rms: dbfs(self.meter.rms()),
                    peak: dbfs(self.meter.peak()),
                };

                match self.sender.try_send(loudness) {
                    Ok(_) => {}
                    Err(TrySendError::Full(_)) => {}
                    Err(TrySendError::Closed(_)) => {
                        // no one is interested anymore, so we shutdown
                        break;
                    }
                }

                self.last_rms = Instant::now();
            }

            if self.last_peak.elapsed() > Duration::from_millis(500) {
                self.meter.reset_peak();
                self.last_peak = Instant::now();
            }

            if self.stop.load(std::sync::atomic::Ordering::Acquire) == true {
                break;
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
