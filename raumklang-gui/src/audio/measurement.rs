use crate::log;

use super::Loudness;

use raumklang_core::loudness;
use ringbuf::{traits::Consumer, HeapCons};

use std::time::{Duration, Instant};

pub struct Measurement {
    last_rms: Instant,
    last_peak: Instant,
    meter: loudness::Meter,
    recording_cons: HeapCons<f32>,
    loudness_sender: tokio::sync::mpsc::Sender<Loudness>,
    data_sender: tokio::sync::mpsc::Sender<Box<[f32]>>,
    stop_receiver: std::sync::mpsc::Receiver<()>,
}

impl Measurement {
    pub fn new(
        loudness_sender: tokio::sync::mpsc::Sender<Loudness>,
        data_sender: tokio::sync::mpsc::Sender<Box<[f32]>>,
        recording_cons: HeapCons<f32>,
        stop_receiver: std::sync::mpsc::Receiver<()>,
    ) -> Self {
        let last_rms = Instant::now();
        let last_peak = Instant::now();

        // FIXME hardcoded sample rate dependency
        let meter = loudness::Meter::new(13230); // 44100samples / 1000ms * 300ms

        Self {
            last_rms,
            last_peak,
            meter,
            recording_cons,
            data_sender,
            loudness_sender,
            stop_receiver,
        }
    }

    pub fn run(mut self) {
        loop {
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

            match self.stop_receiver.try_recv() {
                Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    // process.send(ProcessHandlerMessage::Stop);
                    // worker_state = WorkerState::Idle;
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }

            std::thread::sleep(Duration::from_millis(10));
        }
    }
}
