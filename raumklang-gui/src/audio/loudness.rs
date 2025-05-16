use std::time::{Duration, Instant};

use raumklang_core::{dbfs, LoudnessMeter};
use ringbuf::{traits::Consumer, HeapCons};

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
    pub last_rms: Instant,
    pub last_peak: Instant,
    pub meter: LoudnessMeter,
    pub recording: HeapCons<f32>,
    pub volume_receiver: tokio::sync::mpsc::Receiver<f32>,
    pub sender: tokio::sync::mpsc::Sender<Loudness>,
    pub stop_receiver: std::sync::mpsc::Receiver<()>,
}

impl Test {
    pub fn new(
        sender: tokio::sync::mpsc::Sender<Loudness>,
        volume_receiver: tokio::sync::mpsc::Receiver<f32>,
        stop_receiver: std::sync::mpsc::Receiver<()>,
        recording_cons: HeapCons<f32>,
    ) -> Self {
        let last_rms = Instant::now();
        let last_peak = Instant::now();

        // FIXME hardcoded sample rate dependency
        let meter = LoudnessMeter::new(13230); // 44100samples / 1000ms * 300ms

        Self {
            last_rms,
            last_peak,
            meter,
            recording: recording_cons,
            sender,
            volume_receiver,
            stop_receiver,
        }
    }

    pub fn run(mut self) {
        // FIXME: set volume
        // if let Ok(volume) = test.volume_receiver.try_recv() {
        //     process.send(ProcessHandlerMessage::SetAmplitude(
        //         raumklang_core::volume_to_amplitude(volume),
        //     ));
        // }
        //

        loop {
            let iter = self.recording.pop_iter();
            if self.meter.update_from_iter(iter) {
                self.last_peak = Instant::now();
            }

            if self.last_rms.elapsed() > Duration::from_millis(150) {
                self.sender.try_send(Loudness {
                    rms: dbfs(self.meter.rms()),
                    peak: dbfs(self.meter.peak()),
                });

                self.last_rms = Instant::now();
            }

            if self.last_peak.elapsed() > Duration::from_millis(500) {
                self.meter.reset_peak();
                self.last_peak = Instant::now();
            }

            match self.stop_receiver.try_recv() {
                Ok(()) | Err(std::sync::mpsc::TryRecvError::Disconnected) => {
                    break;
                }
                Err(std::sync::mpsc::TryRecvError::Empty) => {}
            }
        }
    }
}
