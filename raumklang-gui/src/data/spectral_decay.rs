use core::slice;
use std::{collections, fmt};

use iced::task::{sipper, Sipper};
use raumklang_core::{FrequencyResponse, Window, WindowBuilder};
use rustfft::{num_complex::Complex, FftPlanner};

use crate::data::smooth_fractional_octave;

#[derive(Debug, Clone)]
pub enum Event {
    ComputingStarted,
}

#[derive(Clone)]
pub struct SpectralDecay(Vec<raumklang_core::FrequencyResponse>);

impl SpectralDecay {
    pub fn len(&self) -> usize {
        self.0.len()
    }

    pub fn iter(&self) -> slice::Iter<raumklang_core::FrequencyResponse> {
        self.0.iter()
    }

    pub fn into_iter(self) -> impl Iterator<Item = raumklang_core::FrequencyResponse> {
        self.0.into_iter()
    }
}

// impl IntoIterator for SpectralDecay {
//     type Item = raumklang_core::FrequencyResponse;

//     type IntoIter = Vec<raumklang_core::FrequencyResponse> as Iterator;

//     fn into_iter(self) -> Self::IntoIter {
//         self.0.into_iter()
//     }
// }

impl fmt::Debug for SpectralDecay {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Spectral Decay of size: {} slices", self.0.len())
    }
}

pub(crate) fn compute(
    mut ir: raumklang_core::ImpulseResponse,
) -> impl Sipper<SpectralDecay, Event> {
    sipper(move |mut output| async move {
        let sample_rate = ir.sample_rate;
        output.send(Event::ComputingStarted).await;

        let shift = (0.005 * sample_rate as f32).floor() as usize;
        let left_width = (0.1 * sample_rate as f32).floor() as usize;
        // TODO: check if 500 ms is right
        let right_width = (0.5 * sample_rate as f32).floor() as usize;
        let window = WindowBuilder::new(Window::Hann, left_width, Window::Tukey(0.25), right_width);
        let window = window.build();

        ir.data.rotate_right(left_width);

        let mut start = 0;
        let window_size = left_width + right_width;

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(window_size);

        tokio::task::spawn_blocking(move || {
            let mut frequency_responses = vec![];

            while start <= window_size {
                let ir_slice = &ir.data[start..start + window_size];
                let mut windowed_impulse_response: Vec<_> = ir_slice
                    .iter()
                    .copied()
                    .take(window.len())
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

                let smoothed_data = smooth_fractional_octave(&data, 6);
                let data = smoothed_data.iter().map(Complex::from).collect();

                let sample_rate = ir.sample_rate;
                frequency_responses.push(FrequencyResponse { sample_rate, data });

                start += shift;
            }

            SpectralDecay(frequency_responses)
        })
        .await
        .unwrap()
    })
}
