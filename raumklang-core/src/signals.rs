mod noise;
mod sweep;

use std::path::Path;

pub use noise::{PinkNoise, WhiteNoise};
pub use sweep::{ExponentialSweep, LinearSineSweep};

use crate::{Error, WavLoadError};

pub trait FiniteSignal: Send + Sync + ExactSizeIterator<Item = f32> {}

impl<T> FiniteSignal for T where T: Send + Sync + ExactSizeIterator<Item = f32> {}

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

pub(crate) fn map_hound_error(err: hound::Error) -> WavLoadError {
    match err {
        hound::Error::IoError(error) => WavLoadError::Io(error),
        _ => WavLoadError::Other,
    }
}
