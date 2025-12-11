use core::slice;
use std::{fmt, sync::Arc, time::Duration};

use rustfft::{
    num_complex::{Complex, Complex32},
    FftPlanner,
};

use crate::data::{SampleRate, Samples};

pub struct Preferences {
    span_before_peak: Duration,
    span_after_peak: Duration,
    window_width: Duration,
}

#[derive(Clone)]
pub struct Spectrogram {
    pub span_before_peak: Samples,
    pub span_after_peak: Samples,
    slices: Vec<super::FrequencyResponse>,
}

impl Spectrogram {
    pub fn len(&self) -> usize {
        self.slices.len()
    }

    pub fn iter(&self) -> slice::Iter<'_, super::FrequencyResponse> {
        self.slices.iter()
    }
}

pub(crate) async fn compute(
    ir: raumklang_core::ImpulseResponse,
    preferences: Preferences,
) -> Spectrogram {
    let sample_rate = SampleRate::from(ir.sample_rate);

    let window_size: usize = Samples::from_duration(preferences.window_width, sample_rate).into();

    // Gaussian window
    let half_window_size = window_size / 2;
    let sig = 0.3;
    let window: Vec<_> = (0..window_size)
        .map(|n| (n as f32 - half_window_size as f32) / ((sig * window_size as f32) / 2.0))
        .map(|w| f32::powi(w, 2))
        .map(|s| f32::exp(-0.5 * s))
        .collect();

    let span_before_peak = Samples::from_duration(preferences.span_before_peak, sample_rate);
    let span_after_peak = Samples::from_duration(preferences.span_after_peak, sample_rate);

    // ir.data.rotate_right(window_size);

    // let ir: Vec<_> = ir.data.into_iter().collect();
    // zero padding
    dbg!(span_before_peak);
    dbg!(span_after_peak);
    let ir: Vec<_> = (0..half_window_size + usize::from(span_before_peak))
        .map(|_| Complex32::from(0.0))
        .chain(
            ir.data
                .into_iter()
                .take(window_size + usize::from(span_after_peak)),
        )
        .collect();

    let slices = 200;
    let analysed_with = span_before_peak + span_after_peak;
    let shift = usize::from(analysed_with) / (slices - 1);

    dbg!(shift);

    let mut start = 0;
    tokio::task::spawn_blocking(move || {
        let mut slices = Vec::with_capacity(slices);

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(window_size);

        while start + half_window_size < analysed_with.into() {
            let ir_slice = &ir[start..start + window_size];
            let mut windowed_impulse_response: Vec<_> = ir_slice
                .iter()
                .copied()
                .enumerate()
                .map(|(i, s)| s * window[i])
                .collect();

            fft.process(&mut windowed_impulse_response);

            let data_len = windowed_impulse_response.len() / 2 - 1;
            let data: Vec<_> = windowed_impulse_response
                .into_iter()
                .take(data_len)
                .map(Complex::norm)
                .collect();

            slices.push(super::FrequencyResponse {
                sample_rate: sample_rate.into(),
                data: Arc::new(data),
            });

            start += shift;
        }

        Spectrogram {
            span_before_peak,
            span_after_peak,
            slices,
        }
    })
    .await
    .unwrap()
}

impl fmt::Debug for Spectrogram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Spectral Decay of size: {} slices", self.len())
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            span_before_peak: Duration::from_millis(200),
            span_after_peak: Duration::from_millis(1000),
            window_width: Duration::from_millis(500),
        }
    }
}
