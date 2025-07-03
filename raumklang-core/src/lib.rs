mod audio;
mod impulse_response;
pub mod loudness;
pub mod sweep;
mod window;

pub use audio::*;
pub use impulse_response::*;
pub use window::*;

use rand::{distributions, distributions::Distribution, rngs, SeedableRng};
use thiserror::Error;

use std::{
    f32,
    io::{self},
    path::Path,
    slice::Iter,
    time::Duration,
};

#[derive(Error, Debug)]
pub enum Error {
    #[error("error laoding a measurement")]
    WavLoadFile(#[from] WavLoadError),
    #[error(transparent)]
    AudioBackend(#[from] AudioBackendError),
}

#[derive(Error, Debug)]
pub enum WavLoadError {
    #[error(transparent)]
    Io(#[from] io::Error),
    #[error("unknown")]
    Other,
}

#[derive(Debug, Clone)]
pub struct Loopback(Measurement);

#[derive(Debug, Clone)]
pub struct Measurement {
    sample_rate: u32,
    data: Vec<f32>,
}

impl Loopback {
    pub fn new(inner: Measurement) -> Self {
        Self(inner)
    }

    pub fn iter(&self) -> Iter<f32> {
        self.0.iter()
    }

    pub fn sample_rate(&self) -> u32 {
        self.0.sample_rate()
    }

    pub fn duration(&self) -> usize {
        self.0.duration()
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let measurement = Measurement::from_file(path)?;

        Ok(Self(measurement))
    }
}

impl AsRef<Measurement> for Loopback {
    fn as_ref(&self) -> &Measurement {
        &self.0
    }
}

impl Measurement {
    pub fn new(sample_rate: u32, data: Vec<f32>) -> Self {
        Self { sample_rate, data }
    }

    pub fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let mut file = hound::WavReader::open(path).map_err(map_hound_error)?;

        let sample_rate = file.spec().sample_rate;
        let data: Vec<f32> = file
            .samples::<f32>()
            .collect::<Result<Vec<f32>, _>>()
            .map_err(map_hound_error)?;

        Ok(Measurement::new(sample_rate, data))
    }

    pub fn sample_rate(&self) -> u32 {
        self.sample_rate
    }

    pub fn duration(&self) -> usize {
        self.data.len()
    }

    pub fn iter(&self) -> Iter<f32> {
        self.data.iter()
    }
}

fn map_hound_error(err: hound::Error) -> WavLoadError {
    match err {
        hound::Error::IoError(error) => WavLoadError::Io(error),
        _ => WavLoadError::Other,
    }
}

impl From<Loopback> for Measurement {
    fn from(loopback: Loopback) -> Self {
        loopback.0
    }
}

pub enum Signal<F, I>
where
    F: FiniteSignal,
{
    Finite(F),
    Infinite(I),
}

#[derive(Debug, Clone)]
pub struct LinearSineSweep {
    sample_rate: usize,
    sample_index: usize,
    n_samples: usize,
    amplitude: f32,
    frequency: f32,
    delta_frequency: f32,
    phase: f32,
}

// linear sweep
impl LinearSineSweep {
    pub fn new(
        start_frequency: u16,
        end_frequency: u16,
        duration: Duration,
        amplitude: f32,
        sample_rate: usize,
    ) -> Self {
        let n_samples = sample_rate as f32 * duration.as_secs_f32();
        LinearSineSweep {
            sample_rate,
            sample_index: 0,
            n_samples: n_samples as usize,
            amplitude,
            frequency: start_frequency as f32,
            delta_frequency: (end_frequency - start_frequency) as f32 / n_samples,
            phase: 0.0,
        }
    }
}

impl Iterator for LinearSineSweep {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let result = match self.sample_index < self.n_samples {
            true => Some(self.amplitude * f32::sin(self.phase)),
            _ => None,
        };
        self.frequency += self.delta_frequency;
        let delta_phase = 2.0 * std::f32::consts::PI * self.frequency / self.sample_rate as f32;
        self.phase = (self.phase + delta_phase) % (2.0 * std::f32::consts::PI);
        //let frequency = f32::exp(
        //    f32::ln(self.start_frequency) * (1.0 - self.sample_index / self.duration)
        //        + f32::ln(self.end_frequency) * (self.sample_index / self.duration),
        //);

        self.sample_index += 1;

        result
    }
}

impl ExactSizeIterator for LinearSineSweep {
    fn len(&self) -> usize {
        self.n_samples - self.sample_index
    }
}

pub trait FiniteSignal: Send + Sync + ExactSizeIterator<Item = f32> {}

// impl FiniteSignal for LinearSineSweep {}

impl<T> FiniteSignal for T where T: Send + Sync + ExactSizeIterator<Item = f32> {}

#[derive(Debug, Clone)]
pub struct WhiteNoise {
    amplitude: f32,
    rng: rngs::SmallRng,
    distribution: distributions::Uniform<f32>,
}

// TODO: implement log sweep

impl WhiteNoise {
    pub fn with_amplitude(amplitude: f32) -> Self {
        WhiteNoise {
            amplitude,
            rng: rngs::SmallRng::from_entropy(),
            distribution: distributions::Uniform::new_inclusive(-1.0, 1.0),
        }
    }

    pub fn take_duration(self, sample_rate: usize, duration: usize) -> std::iter::Take<WhiteNoise> {
        self.into_iter().take(sample_rate * duration)
    }
}

impl Default for WhiteNoise {
    fn default() -> Self {
        Self::with_amplitude(1.0)
    }
}

impl Iterator for WhiteNoise {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.amplitude * self.distribution.sample(&mut self.rng);
        Some(sample)
    }
}

impl ExactSizeIterator for WhiteNoise {}
// impl FiniteSignal for std::iter::Take<WhiteNoise> {}

#[derive(Debug, Clone)]
pub struct PinkNoise {
    b0: f32,
    b1: f32,
    b2: f32,
    white_noise: WhiteNoise,
}

impl PinkNoise {
    pub fn with_amplitude(amplitude: f32) -> Self {
        let white_noise = WhiteNoise::with_amplitude(amplitude);

        PinkNoise {
            b0: 0f32,
            b1: 0f32,
            b2: 0f32,
            white_noise,
        }
    }

    pub fn take_duration(self, sample_rate: usize, duration: usize) -> std::iter::Take<PinkNoise> {
        self.into_iter().take(sample_rate * duration)
    }
}

impl Default for PinkNoise {
    fn default() -> Self {
        Self::with_amplitude(1.0)
    }
}

impl Iterator for PinkNoise {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let white = self.white_noise.next().unwrap();
        self.b0 = 0.99765 * self.b0 + white * 0.0990460;
        self.b1 = 0.96300 * self.b1 + white * 0.2965164;
        self.b2 = 0.57000 * self.b2 + white * 1.0526913;
        Some(self.b0 + self.b1 + self.b2 + white * 0.1848)
    }
}

impl ExactSizeIterator for PinkNoise {}
// impl FiniteSignal for std::iter::Take<PinkNoise> {}

pub fn volume_to_amplitude(volume: f32) -> f32 {
    assert!((0.0..=1.0).contains(&volume));

    // FIXME:
    // 1. remove magic numbers
    // https://www.dr-lex.be/info-stuff/volumecontrols.html
    let a = 0.001;
    let b = 6.908;

    if volume < 0.1 {
        volume * 10.0 * a * f32::exp(0.1 * b)
    } else {
        a * f32::exp(b * volume)
    }
}

pub fn write_signal_to_file(
    signal: Box<dyn FiniteSignal<Item = f32>>,
    path: &Path,
) -> Result<(), Error> {
    let sample_rate = 44_100;

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate,
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let mut writer = hound::WavWriter::create(path, spec).map_err(map_hound_error)?;

    for s in signal {
        writer.write_sample(s).map_err(map_hound_error)?;
    }

    writer.finalize().map_err(map_hound_error)?;

    Ok(())
}

#[inline]
pub fn dbfs(v: f32) -> f32 {
    20.0 * f32::log10(v.abs())
}
