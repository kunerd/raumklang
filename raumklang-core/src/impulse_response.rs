use rustfft::{
    num_complex::{Complex, Complex32},
    FftPlanner,
};

use crate::{Error, Loopback, Measurement};

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub sample_rate: u32,
    pub data: Vec<Complex32>,
    pub loopback_fft: Vec<Complex<f32>>,
    pub response_fft: Vec<Complex<f32>>,
}

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub sample_rate: u32,
    pub data: Vec<Complex32>,
}

impl ImpulseResponse {
    pub fn from_signals(loopback: &Loopback, response: &Measurement) -> Result<Self, Error> {
        let sample_rate = loopback.0.sample_rate;
        assert!(sample_rate == response.sample_rate());

        let mut loopback = loopback.0.data.clone();
        let mut response = response.data.clone();

        let response_len = response.len();
        let loopback_len = loopback.len();

        // make record and sweep the same length
        if response_len > loopback_len {
            loopback.append(&mut vec![0.0; response_len - loopback_len]);
        } else {
            response.append(&mut vec![0.0; loopback_len - response_len]);
        }

        assert!(response.len() == loopback.len());

        // double the size
        response.append(&mut vec![0.0; response.len()]);
        loopback.append(&mut vec![0.0; loopback.len()]);

        // convert to complex
        let mut response: Vec<_> = response.iter().map(Complex::from).collect();
        let mut loopback: Vec<_> = loopback.iter().map(Complex::from).collect();

        // convert into frequency domain
        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(response.len());

        fft.process(&mut response);
        fft.process(&mut loopback);

        // devide both
        let mut result: Vec<Complex<f32>> = response
            .iter()
            .zip(loopback.iter())
            .map(|(r, l)| r / l)
            .collect();

        // back to time domain
        let fft = planner.plan_fft_inverse(result.len());
        fft.process(&mut result);

        let scale: f32 = 1.0 / (result.len() as f32);
        let impulse_response: Vec<_> = result.into_iter().map(|s| s.scale(scale)).collect();

        Ok(Self {
            sample_rate,
            data: impulse_response,
            loopback_fft: loopback,
            response_fft: response,
        })
    }

    pub fn from_files(loopback_path: &str, measurment_path: &str) -> Result<Self, Error> {
        let loopback = Loopback::from_file(loopback_path)?;
        let measurement = Measurement::from_file(measurment_path)?;

        Self::from_signals(&loopback, &measurement)
    }
}

impl FrequencyResponse {
    pub fn new(impulse_response: ImpulseResponse, window: &[f32]) -> Self {
        let mut windowed_impulse_response: Vec<_> = impulse_response
            .data
            .iter()
            .take(window.len())
            .enumerate()
            .map(|(i, s)| s * window[i])
            .collect();

        let mut planner = FftPlanner::<f32>::new();
        let fft = planner.plan_fft_forward(windowed_impulse_response.len());

        fft.process(&mut windowed_impulse_response);

        let data_len = windowed_impulse_response.len() / 2 - 1;
        let data = windowed_impulse_response
            .into_iter()
            .take(data_len)
            .collect();

        let sample_rate = impulse_response.sample_rate;
        Self { sample_rate, data }
    }
}
