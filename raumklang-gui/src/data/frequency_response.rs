use super::{impulse_response, ImpulseResponse, Samples, Window};

use iced::task::Sipper;
use ndarray::{concatenate, Array, Array1, ArrayView, Axis};
use ndarray_interp::interp1d::{cubic_spline::CubicSpline, Interp1DBuilder};
use rustfft::num_complex::Complex;

#[derive(Debug, Clone)]
pub struct FrequencyResponse {
    pub origin: raumklang_core::FrequencyResponse,
    pub smoothed: Vec<Complex<f32>>,
}

#[derive(Debug)]
pub enum State {
    Computing,
    Computed(FrequencyResponse),
}

pub struct Computation {
    from: CompputationType,
    window: Window<Samples>,
}

enum CompputationType {
    ImpulseResponse(usize, ImpulseResponse),
    Computation(impulse_response::Computation),
}

impl Computation {
    pub fn from_impulse_response(
        id: usize,
        impulse_response: ImpulseResponse,
        window: Window<Samples>,
    ) -> Self {
        Self {
            from: CompputationType::ImpulseResponse(id, impulse_response),
            window,
        }
    }

    pub fn from_impulse_response_computation(
        computation: impulse_response::Computation,
        window: Window<Samples>,
    ) -> Self {
        Self {
            from: CompputationType::Computation(computation),
            window,
        }
    }

    pub fn run(self) -> impl Sipper<(usize, FrequencyResponse), (usize, ImpulseResponse)> {
        iced::task::sipper(async move |mut progress| {
            let (id, impulse_response) = match self.from {
                CompputationType::ImpulseResponse(id, impulse_response) => (id, impulse_response),
                CompputationType::Computation(computation) => computation.run().await.unwrap(),
            };

            progress.send((id, impulse_response.clone())).await;

            let mut impulse_response = impulse_response.origin;
            let offset = self.window.offset().into();

            impulse_response.data.rotate_right(offset);

            let window: Vec<_> = self.window.curve().map(|(_x, y)| y).collect();
            let frequency_response = tokio::task::spawn_blocking(move || {
                raumklang_core::FrequencyResponse::new(impulse_response, &window)
            })
            .await
            .unwrap();

            (id, FrequencyResponse::new(frequency_response))
        })
    }
}

impl FrequencyResponse {
    pub fn new(origin: raumklang_core::FrequencyResponse) -> Self {
        let smoothed = nth_octave_smoothing(
            &origin.data.iter().map(|s| s.re.abs()).collect::<Vec<f32>>(),
            6,
        );
        let smoothed = smoothed.iter().map(Complex::from).collect();
        Self { origin, smoothed }
    }
}

pub fn nth_octave_smoothing(signal: &[f32], num_fractions: usize) -> Vec<f32> {
    let signal = ArrayView::from(signal);
    let len = signal.len() as f32;
    let n_lin = Array::range(0., len as f32, 1.0);

    //     n_log = N**(n_lin/(N-1))
    let n_log = n_lin.mapv(|n| n / (len - 1.0)).mapv(|n| len.powf(n));

    //     # frequency bin spacing in octaves: log2(n_log[n]/n_log[n-1])
    //     # Note: n_log[0] -> 1
    //     delta_n = np.log2(n_log[1])
    let delta_n = n_log[1].log2();

    //     # width of the window in logarithmically spaced samples
    //     # Note: Forcing the window to have an odd length increases the deviation
    //     #       from the exact width, but makes sure that the delay introduced in
    //     #       the convolution is integer and can be easily compensated
    //     n_window = int(2 * np.floor(1 / (num_fractions * delta_n * 2)) + 1)
    let n_window = (2.0 * (1.0 / (num_fractions as f32 * delta_n * 2.0)).floor() + 1.0) as usize;

    //     if n_window == 1:
    //         raise ValueError((
    //             "The smoothing width given by num_fractions is below the frequency"
    //             " resolution of the signal. Increase the signal length or decrease"
    //             " num_fractions"))
    if n_window == 1 {
        panic!("num_fraction below frequency");
    }

    //     # generate the smoothing window
    //     if isinstance(window, str):
    //         window = sgn.windows.get_window(window, n_window, fftbins=False)
    //     elif isinstance(window, (list, np.ndarray)):
    //         # undocumented possibility for testing
    //         window = np.asanyarray(window, dtype=float)
    //         if window.shape != (n_window, ):
    //             raise ValueError(
    //                 f"window.shape is {window.shape} but must be ({n_window}, )")
    //     else:
    //         raise ValueError(f"window is of type {str(type(window))} but must be "
    //                          "of type string")
    let window = Array1::<f32>::ones(n_window);

    //     for nn in range(len(data)):
    //         # interpolate to logarithmically spaced frequencies
    //         interpolator = interp1d(
    //             n_lin + 1, data[nn], "cubic", copy=False, assume_sorted=True)
    //         data[nn] = interpolator(n_log)
    let interpolator = Interp1DBuilder::new(signal)
        .strategy(CubicSpline::new())
        .x(n_lin.clone() + 1.0)
        .build()
        .unwrap();
    let result = interpolator.interp_array(&n_log).unwrap();
    //         # apply a moving average filter based on the window function
    //         data[nn] = generic_filter1d(
    //             data[nn],
    //             function=_weighted_moving_average,
    //             filter_size=n_window,
    //             mode='nearest',
    //             extra_arguments=(window,))
    let result: Array1<f32> = result
        .windows(n_window)
        .into_iter()
        .map(|d| d.mean().unwrap())
        .collect();

    //         # interpolate to original frequency axis
    //         interpolator = interp1d(
    //             n_log, data[nn], "cubic", copy=False, assume_sorted=True)
    //         data[nn] = interpolator(n_lin + 1)

    // FIXME: ugly hack, use nearest here
    let last = result.last().unwrap();
    let result = concatenate![
        Axis(0),
        result,
        Array1::from_elem(n_log.len() - result.len(), *last),
    ];

    let interpolator = Interp1DBuilder::new(result)
        .strategy(CubicSpline::new())
        .x(n_log)
        .build()
        .unwrap();
    let result = interpolator.interp_array(&(n_lin + 1.0)).unwrap();
    //     # generate return signal --------------------------------------------------
    //     if mode == "magnitude_zerophase":
    //         data = data[0]
    //     elif mode == "complex":
    //         data = data[0] + 1j * data[1]
    //     elif mode == "magnitude_phase":
    //         data = data[0] * np.exp(1j * data[1])
    //     elif mode == "magnitude":
    //         data = data[0] * np.exp(1j * np.angle(signal.freq_raw))

    //     # force 0 Hz and Nyquist to be real if it might not be the case
    //     if mode in ["complex", "magnitude_phase", "magnitude"]:
    //         data[..., 0] = np.abs(data[..., 0])
    //         data[..., -1] = np.abs(data[..., -1])

    //     signal = signal.copy()
    //     signal.freq_raw = data

    //     return signal, (n_window, 1 / (n_window * delta_n))
    result.to_vec()
}
// def smooth_fractional_octave(signal, num_fractions, mode="magnitude_zerophase",
//                              window="boxcar"):
//     """
//     Smooth spectrum with a fractional octave width.

//     The smoothing is done according to Tylka et al. 2017 [#]_ (method 2) in
//     three steps:

//     1. Interpolate the spectrum to a logarithmically spaced frequency scale
//     2. Smooth the spectrum by convolution with a smoothing window
//     3. Interpolate the spectrum to the original linear frequency scale

//     Smoothing of complex-valued time data is not implemented.

//     Parameters
//     ----------
//     signal : pyfar.Signal
//         The input data.
//     num_fractions : number
//         The width of the smoothing window in fractional octaves, e.g., 3 will
//         apply third octave smoothing and 1 will apply octave smoothing.
//     mode : str, optional
//         ``"magnitude_zerophase"``
//             Only the magnitude response, i.e., the absolute spectrum is
//             smoothed. Note that this return a zero-phase signal. It might be
//             necessary to generate a minimum or linear phase if the data is
//             subject to further processing after the smoothing (cf.
//             :py:func:`~pyfar.dsp.minimum_phase` and
//             :py:func:`~pyfar.dsp.linear_phase`)
//         ``"magnitude"``
//             Smooth the magnitude and keep the phase of the input signal.
//         ``"magnitude_phase"``
//             Separately smooth the magnitude and unwrapped phase response.
//         ``"complex"``
//             Separately smooth the real and imaginary part of the spectrum.

//         Note that the modes `magnitude_zerophase` and `magnitude` make sure
//         that the smoothed magnitude response is as expected at the cost of an
//         artificial phase response. This is often desired, e.g., when plotting
//         signals or designing compensation filters. The modes `magnitude_phase`
//         and `complex` smooth all information but might cause a high frequency
//         energy loss in the smoothed magnitude response. The default is
//         ``"magnitude_zerophase"``.
//     window : str, optional
//         String that defines the smoothing window. All windows from
//         :py:func:`~pyfar.dsp.time_window` that do not require an additional
//         parameter can be used. The default is "boxcar", which uses the
//         most commonly used rectangular window.

//     Returns
//     -------
//     signal : pyfar.Signal
//         The smoothed output data
//     window_stats : tuple
//         A tuple containing information about the smoothing process

//         `n_window`
//             The window length in (logarithmically spaced) samples
//         `num_fractions`
//             The actual width of the window in fractional octaves. This can
//             deviate from the desired width because the smoothing window must
//             have an integer sample length

//     Notes
//     -----
//     Method 3 in Tylka at al. 2017 is mathematically more elegant at the
//     price of a largely increased computational and memory cost. In most
//     practical cases, methods 2 and 3 yield close to identical results (cf. Fig.
//     2 and 3 in Tylka et al. 2017). If the spectrum contains extreme
//     discontinuities, however, method 3 is superior (see examples below).

//     References
//     ----------
//     .. [#] J. G. Tylka, B. B. Boren, and E. Y. Choueiri, “A Generalized Method
//            for Fractional-Octave Smoothing of Transfer Functions that Preserves
//            Log-Frequency Symmetry (Engineering Report),” J. Audio Eng. Soc. 65,
//            239-245 (2017). doi:10.17743/jaes.2016.0053

//     Examples
//     --------
//     Octave smoothing of continuous spectrum consisting of two bell filters.

//     .. plot::

//         >>> import pyfar as pf
//         >>> signal = pf.signals.impulse(441)
//         >>> signal = pf.dsp.filter.bell(signal, 1e3, 12, 1, "III")
//         >>> signal = pf.dsp.filter.bell(signal, 10e3, -60, 100, "III")
//         >>> smoothed, _ = pf.dsp.smooth_fractional_octave(signal, 1)
//         >>> ax = pf.plot.freq(signal, label="input")
//         >>> pf.plot.freq(smoothed, label="smoothed")
//         >>> ax.legend(loc=3)

//     Octave smoothing of the discontinuous spectrum of a sine signal causes
//     artifacts at the edges due to the intermediate interpolation steps (cf.
//     Tylka et al. 2017, Fig. 4). However this is a rather unusual application
//     and is mentioned only for the sake of completeness.

//     .. plot::

//         >>> import pyfar as pf
//         >>> signal = pf.signals.sine(1e3, 4410)
//         >>> signal.fft_norm = "amplitude"
//         >>> smoothed, _ = pf.dsp.smooth_fractional_octave(signal, 1)
//         >>> ax = pf.plot.freq(signal, label="input")
//         >>> pf.plot.freq(smoothed, label="smoothed")
//         >>> ax.set_xlim(200, 4e3)
//         >>> ax.set_ylim(-45, 5)
//         >>> ax.legend(loc=3)
//     """

//     if not isinstance(signal, pf.Signal):
//         raise TypeError("Input signal has to be of type pyfar.Signal")

//     if type(signal) is not pf.FrequencyData and signal.complex:
//         raise TypeError(("Fractional octave smoothing for complex-valued "
//                          "time data is not implemented."))

//     if mode in ["magnitude_zerophase", "magnitude"]:
//         data = [np.atleast_2d(np.abs(signal.freq_raw))]
//     elif mode == "complex":
//         data = [np.atleast_2d(np.real(signal.freq_raw)),
//                 np.atleast_2d(np.imag(signal.freq_raw))]
//     elif mode == "magnitude_phase":
//         data = [np.atleast_2d(np.abs(signal.freq_raw)),
//                 np.atleast_2d(pf.dsp.phase(signal, unwrap=True))]
//     else:
//         raise ValueError((f"mode is '{mode}' but must be 'magnitude_zerophase'"
//                           ", 'magnitude_phase', 'magnitude', or 'complex'"))

//     # linearly and logarithmically spaced frequency bins ----------------------
//     N = signal.n_bins
//     n_lin = np.arange(N)
//     n_log = N**(n_lin/(N-1))

//     # frequency bin spacing in octaves: log2(n_log[n]/n_log[n-1])
//     # Note: n_log[0] -> 1
//     delta_n = np.log2(n_log[1])

//     # width of the window in logarithmically spaced samples
//     # Note: Forcing the window to have an odd length increases the deviation
//     #       from the exact width, but makes sure that the delay introduced in
//     #       the convolution is integer and can be easily compensated
//     n_window = int(2 * np.floor(1 / (num_fractions * delta_n * 2)) + 1)

//     if n_window == 1:
//         raise ValueError((
//             "The smoothing width given by num_fractions is below the frequency"
//             " resolution of the signal. Increase the signal length or decrease"
//             " num_fractions"))

//     # generate the smoothing window
//     if isinstance(window, str):
//         window = sgn.windows.get_window(window, n_window, fftbins=False)
//     elif isinstance(window, (list, np.ndarray)):
//         # undocumented possibility for testing
//         window = np.asanyarray(window, dtype=float)
//         if window.shape != (n_window, ):
//             raise ValueError(
//                 f"window.shape is {window.shape} but must be ({n_window}, )")
//     else:
//         raise ValueError(f"window is of type {str(type(window))} but must be "
//                          "of type string")

//     for nn in range(len(data)):
//         # interpolate to logarithmically spaced frequencies
//         interpolator = interp1d(
//             n_lin + 1, data[nn], "cubic", copy=False, assume_sorted=True)
//         data[nn] = interpolator(n_log)

//         # apply a moving average filter based on the window function
//         data[nn] = generic_filter1d(
//             data[nn],
//             function=_weighted_moving_average,
//             filter_size=n_window,
//             mode='nearest',
//             extra_arguments=(window,))

//         # interpolate to original frequency axis
//         interpolator = interp1d(
//             n_log, data[nn], "cubic", copy=False, assume_sorted=True)
//         data[nn] = interpolator(n_lin + 1)

//     # generate return signal --------------------------------------------------
//     if mode == "magnitude_zerophase":
//         data = data[0]
//     elif mode == "complex":
//         data = data[0] + 1j * data[1]
//     elif mode == "magnitude_phase":
//         data = data[0] * np.exp(1j * data[1])
//     elif mode == "magnitude":
//         data = data[0] * np.exp(1j * np.angle(signal.freq_raw))

//     # force 0 Hz and Nyquist to be real if it might not be the case
//     if mode in ["complex", "magnitude_phase", "magnitude"]:
//         data[..., 0] = np.abs(data[..., 0])
//         data[..., -1] = np.abs(data[..., -1])

//     signal = signal.copy()
//     signal.freq_raw = data

//     return signal, (n_window, 1 / (n_window * delta_n))
