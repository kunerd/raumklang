use core::slice;
use std::{fmt, sync::Arc, time::Duration};

use raumklang_core::{Window, WindowBuilder};
use rustfft::{
    num_complex::{Complex, Complex32},
    FftPlanner,
};

use crate::data::{smooth_fractional_octave, SampleRate, Samples};

pub struct Preferences {
    shift: Duration,
    left_window_width: Duration,
    right_window_width: Duration,
    smoothing_fraction: u8,
}

#[derive(Clone)]
pub struct SpectralDecay(Vec<super::FrequencyResponse>);

impl SpectralDecay {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> slice::Iter<'_, super::FrequencyResponse> {
        self.0.iter()
    }
}

pub(crate) async fn compute(
    ir: raumklang_core::ImpulseResponse,
    preferences: Preferences,
) -> SpectralDecay {
    let sample_rate = SampleRate::from(ir.sample_rate);

    let shift: usize = Samples::from_duration(preferences.shift, sample_rate).into();
    let left_width = Samples::from_duration(preferences.left_window_width, sample_rate);
    let right_width = Samples::from_duration(preferences.right_window_width, sample_rate);
    let analysis_width = Samples::from_duration(Duration::from_millis(300), sample_rate);

    let window = WindowBuilder::new(
        Window::Hann,
        left_width.into(),
        Window::Tukey(0.25),
        right_width.into(),
    );
    let window = window.build();

    let ir: Vec<_> = (0..usize::from(left_width))
        .map(|_| Complex32::from(0.0))
        .chain(
            ir.data
                .into_iter()
                .take(usize::from(analysis_width + right_width)),
        )
        .collect();

    let mut start = 0;
    let window_size = usize::from(left_width + right_width);

    let analysis_width: usize = analysis_width.into();

    tokio::task::spawn_blocking(move || {
        let mut frequency_responses =
            Vec::with_capacity((analysis_width - usize::from(left_width)) / shift);

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(window_size);

        while start + usize::from(left_width) < analysis_width {
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

            let data = smooth_fractional_octave(&data, preferences.smoothing_fraction);

            frequency_responses.push(super::FrequencyResponse {
                sample_rate: u32::from(sample_rate),
                data: Arc::new(data),
            });

            start += shift;
        }

        SpectralDecay(frequency_responses)
    })
    .await
    .unwrap()
}

impl fmt::Debug for SpectralDecay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Spectral Decay of size: {} slices", self.0.len())
    }
}

impl Default for Preferences {
    fn default() -> Self {
        Self {
            shift: Duration::from_millis(20),
            left_window_width: Duration::from_millis(100),
            right_window_width: Duration::from_millis(400),
            smoothing_fraction: 24,
        }
    }
}
