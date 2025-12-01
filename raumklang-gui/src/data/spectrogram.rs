use core::slice;
use std::{fmt, sync::Arc, time::Duration};

use iced::task::{sipper, Sipper};
use raumklang_core::{Window, WindowBuilder};
use rustfft::{num_complex::Complex, FftPlanner};

use crate::data::{SampleRate, Samples};

#[derive(Debug, Clone)]
pub enum Event {
    ComputingStarted,
}

pub struct Preferences {
    // shift: Duration,
    span_before_peak: Duration,
    span_after_peak: Duration,
    window_width: Duration,
    // smoothing_fraction: u8,
}

#[derive(Clone)]
pub struct Spectrogram(Vec<super::FrequencyResponse>);

impl Spectrogram {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> slice::Iter<'_, super::FrequencyResponse> {
        self.0.iter()
    }
}

pub(crate) fn compute(
    mut ir: raumklang_core::ImpulseResponse,
    preferences: Preferences,
) -> impl Sipper<Spectrogram, Event> {
    sipper(move |mut output| async move {
        let sample_rate = SampleRate::from(ir.sample_rate);

        output.send(Event::ComputingStarted).await;

        let window_size: usize =
            Samples::from_duration(preferences.window_width, sample_rate).into();

        // Gaussian window
        let half_window_size = (window_size / 2) as f32;
        let sig = 0.5;
        let window: Vec<_> = (0..window_size)
            .into_iter()
            .map(|n| (n as f32 - half_window_size) / (sig * half_window_size))
            .map(|w| f32::powi(w, 2))
            .map(|s| f32::exp(-0.5 * s))
            .collect();

        // let window =
        //     WindowBuilder::new(Window::Hann, window_size / 2, Window::Hann, window_size / 2);
        // let window = window.build();

        let span_before_peak = Samples::from_duration(preferences.span_before_peak, sample_rate);
        let span_after_peak = Samples::from_duration(preferences.span_after_peak, sample_rate);

        ir.data.rotate_right(span_before_peak.into());

        let analysed_with = span_before_peak + span_after_peak;
        let slices = 200;
        let shift = usize::from(analysed_with) / (slices - 1);

        let mut start = 0;
        tokio::task::spawn_blocking(move || {
            let mut frequency_responses = Vec::with_capacity(slices);

            let mut planner = FftPlanner::<f32>::new();
            let fft = planner.plan_fft_forward(window_size);

            while start < analysed_with.into() {
                let ir_slice = &ir.data[start..start + window_size];
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

                let sample_rate = ir.sample_rate;
                frequency_responses.push(super::FrequencyResponse {
                    sample_rate,
                    data: Arc::new(data),
                });

                start += shift;
            }

            Spectrogram(frequency_responses)
        })
        .await
        .unwrap()
    })
}

impl fmt::Debug for Spectrogram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Spectral Decay of size: {} slices", self.0.len())
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
