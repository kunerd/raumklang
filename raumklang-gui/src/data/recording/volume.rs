use crate::audio;

#[derive(Debug, Clone, PartialEq, PartialOrd)]
pub struct Volume(f32);

impl Volume {
    pub fn new(volume: f32, loudness: &audio::Loudness) -> Result<Volume, ValidationError> {
        let rms = loudness.rms;

        if rms < -14.0 {
            return Err(ValidationError::ToLow(rms));
        }

        if loudness.rms >= -10.0 {
            return Err(ValidationError::ToHigh(rms));
        }

        Ok(Volume(volume))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum ValidationError {
    #[error("loudness of {0}, is to low.")]
    ToLow(f32),
    #[error("loudness of {0}, is to high.")]
    ToHigh(f32),
}
