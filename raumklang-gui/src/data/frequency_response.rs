use std::sync::Arc;

use iced::task::{sipper, Sipper};
use ndarray::{concatenate, Array, Array1, ArrayView, Axis};
use ndarray_interp::interp1d::{cubic_spline::CubicSpline, Interp1DBuilder};
use ndarray_stats::SummaryStatisticsExt;

use super::{Samples, Window};

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub sample_rate: u32,
    pub data: Arc<Vec<f32>>,
}

impl FrequencyResponse {
    pub fn from_data(frequency_response: raumklang_core::FrequencyResponse) -> Self {
        let sample_rate = frequency_response.sample_rate;
        let data = frequency_response
            .data
            .into_iter()
            .map(|s| s.re.abs())
            .collect();

        Self {
            sample_rate,
            data: Arc::new(data),
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    ComputingStarted,
}

pub fn compute(
    mut impulse_response: raumklang_core::ImpulseResponse,
    window: Window<Samples>,
) -> impl Sipper<FrequencyResponse, Event> {
    sipper(|mut output| async move {
        output.send(Event::ComputingStarted).await;

        let offset = window.offset().into();

        impulse_response.data.rotate_right(offset);

        let window: Vec<_> = window.curve().map(|(_x, y)| y).collect();

        tokio::task::spawn_blocking(move || {
            raumklang_core::FrequencyResponse::new(impulse_response, &window)
        })
        .await
        .map(FrequencyResponse::from_data)
        .unwrap()
    })
}

//   credits to https://github.com/pyfar/pyfar
//
//   References
//   ----------
//   .. [#] J. G. Tylka, B. B. Boren, and E. Y. Choueiri, “A Generalized Method
//          for Fractional-Octave Smoothing of Transfer Functions that Preserves
//          Log-Frequency Symmetry (Engineering Report),” J. Audio Eng. Soc. 65,
//          239-245 (2017). doi:10.17743/jaes.2016.0053
// TODO: add implementation for complex numbers
pub fn smooth_fractional_octave(signal: &[f32], num_fractions: u8) -> Vec<f32> {
    let signal = ArrayView::from(signal);

    // linearly and logarithmically spaced frequency bins ----------------------
    let len = signal.len() as f32;
    let n_lin = Array::range(0., len as f32, 1.0);
    let n_log = n_lin.mapv(|n| n / (len - 1.0)).mapv(|n| len.powf(n));

    // frequency bin spacing in octaves: log2(n_log[n]/n_log[n-1])
    // Note: n_log[0] -> 1
    let delta_n = n_log[1].log2();

    // width of the window in logarithmically spaced samples
    // Note: Forcing the window to have an odd length increases the deviation
    //       from the exact width, but makes sure that the delay introduced in
    //       the convolution is integer and can be easily compensated
    let n_window = (2.0 * (1.0 / (num_fractions as f32 * delta_n * 2.0)).floor() + 1.0) as usize;

    // FIXME return error
    if n_window == 1 {
        panic!("num_fraction below frequency");
    }

    // boxcar window
    let window = Array1::<f32>::ones(n_window);

    // interpolate to logarithmically spaced frequencies
    let interpolator = Interp1DBuilder::new(signal)
        .strategy(CubicSpline::new())
        .x(n_lin.clone() + 1.0)
        .build()
        .unwrap();
    let result = interpolator.interp_array(&n_log).unwrap();

    // add padding nearest value to start and end
    let first = result.first().unwrap();
    let last = result.last().unwrap();
    let half_window_size = n_window / 2;

    let result = concatenate![
        Axis(0),
        Array1::from_elem(half_window_size, *first),
        result,
        Array1::from_elem(half_window_size, *last),
    ];

    // apply a moving average filter based on the window function
    let result: Array1<f32> = result
        .windows(n_window)
        .into_iter()
        .map(|d| d.weighted_mean(&window.view()).unwrap())
        .collect();

    // interpolate to original frequency axis
    let interpolator = Interp1DBuilder::new(result)
        .strategy(CubicSpline::new())
        .x(n_log)
        .build()
        .unwrap();
    let result = interpolator.interp_array(&(n_lin + 1.0)).unwrap();

    // TODO: return window stats
    result.to_vec()
}
