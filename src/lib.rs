use rand::{distributions, distributions::Distribution, rngs, SeedableRng};

//use anyhow::anyhow;
//use ndarray::{Array, Axis};
//use ndarray_stats::QuantileExt;
use ringbuf::HeapRb;

//use plotters::prelude::*;

use std::{
    //    fs::File,
    //    io::{self, BufWriter, Write},
    io::{self, Write},
    //    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

//use rustfft::{num_complex::Complex, FftPlanner};

pub struct SineSweep {
    sample_rate: u32,
    sample_index: u32,
    amplitude: f32,
    frequency: f32,
    delta_frequency: f32,
    phase: f32,
    duration: u32,
}

// linear sweep
// TODO: implement log sweep
impl SineSweep {
    pub fn new(
        start_frequency: u16,
        end_frequency: u16,
        duration: u32,
        amplitude: f32,
        sample_rate: u32,
    ) -> Self {
        SineSweep {
            sample_rate,
            sample_index: 0,
            amplitude,
            frequency: start_frequency as f32,
            delta_frequency: (end_frequency - start_frequency) as f32
                / (sample_rate * duration) as f32,
            phase: 0.0,
            duration,
        }
    }
}

impl Iterator for SineSweep {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let n_samples = self.sample_rate * self.duration;
        let result = match self.sample_index < n_samples {
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

pub struct WhiteNoise {
    amplitude: f32,
    rng: rngs::SmallRng,
    distribution: distributions::Uniform<f32>,
}

impl WhiteNoise {
    pub fn with_amplitude(amplitude: f32) -> Self {
        WhiteNoise {
            amplitude,
            rng: rngs::SmallRng::from_entropy(),
            distribution: distributions::Uniform::new_inclusive(-1.0, 1.0),
        }
    }
}

impl Iterator for WhiteNoise {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        let sample = self.amplitude * self.distribution.sample(&mut self.rng);
        Some(sample)
    }
}

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

pub struct PlaySignalConfig<'a> {
    pub out_port_name: &'a str,
    pub dest_port_names: Vec<&'a str>,
    pub duration: u8,
    pub volume: f32,
}

fn volume_to_amplitude(volume: f32) -> f32 {
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

pub fn play_signal<D>(
    audio: D,
    config: &PlaySignalConfig,
    duration: Option<Duration>,
) -> Result<(), anyhow::Error>
where
    D: IntoIterator<Item = f32>,
    <D as IntoIterator>::IntoIter: 'static + Send,
{
    let jack_client_name = "raumklang";

    // TODO use app name
    let (client, _status) =
        jack::Client::new(jack_client_name, jack::ClientOptions::NO_START_SERVER)?;

    // TODO remove hard coded port name
    let mut out_port = client.register_port(config.out_port_name, jack::AudioOut::default())?;

    let mut audio = audio.into_iter();
    let (complete_tx, complete_rx) = std::sync::mpsc::sync_channel(1);
    let process = jack::ClosureProcessHandler::new(
        move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
            // Get output buffer
            let out = out_port.as_mut_slice(ps);

            // Write output
            for o in out.iter_mut() {
                if let Some(sample) = audio.next() {
                    *o = sample;
                } else {
                    complete_tx.try_send(()).ok();
                    *o = 0.0f32;
                }
            }

            // Continue as normal
            jack::Control::Continue
        },
    );

    // 4. Activate the client. Also connect the ports to the system audio.
    let active_client = client.activate_async((), process)?;
    let client_out_port = format!("{}:{}", jack_client_name, config.out_port_name);

    for dest in &config.dest_port_names {
        active_client
            .as_client()
            .connect_ports_by_name(&client_out_port, dest)?;
    }

    if let Some(t) = duration {
        std::thread::sleep(t);
    } else {
        let start_time = Instant::now();
        complete_rx.recv().unwrap();
        let duration = start_time.elapsed();
        println!("Playback time: {:?}", duration);
    }

    Ok(())
}

pub fn play_white_noise(config: &PlaySignalConfig) -> anyhow::Result<()> {
    let amplitude = volume_to_amplitude(config.volume);
    let white_noise = WhiteNoise::with_amplitude(amplitude);

    play_signal(
        white_noise,
        config,
        Some(Duration::from_secs(config.duration as u64)),
    )
}

pub fn play_pink_noise(config: &PlaySignalConfig) -> anyhow::Result<()> {
    let amplitude = volume_to_amplitude(config.volume);
    let pink_noise = PinkNoise::with_amplitude(amplitude);

    play_signal(
        pink_noise,
        config,
        Some(Duration::from_secs(config.duration as u64)),
    )
}

pub fn play_linear_sine_sweep(
    start_frequency: u16,
    max_frequency: u16,
    config: &PlaySignalConfig,
) -> anyhow::Result<()> {
    // TODO make configurable
    let sample_rate = 48_000;

    let amplitude = volume_to_amplitude(config.volume);
    let sine_sweep = SineSweep::new(
        start_frequency,
        max_frequency,
        config.duration.into(),
        amplitude,
        sample_rate,
    );

    play_signal(sine_sweep, config, None)
}

pub fn meter_rms(input_port_name: &str) -> anyhow::Result<()> {
    let (client, _status) = jack::Client::new("raumklang", jack::ClientOptions::NO_START_SERVER)?;

    let in_port = client.register_port("rms_in", jack::AudioIn::default())?;

    let buf_size = 512;
    let rb = HeapRb::<_>::new(buf_size);
    let (mut prod, mut cons) = rb.split();

    let process_callback = move |_: &jack::Client, ps: &jack::ProcessScope| -> jack::Control {
        let in_a_p = in_port.as_slice(ps);

        for i in in_a_p {
            prod.push(*i).unwrap();
        }

        jack::Control::Continue
    };
    let process = jack::ClosureProcessHandler::new(process_callback);

    let active_client = client.activate_async((), process)?;

    active_client
        .as_client()
        .connect_ports_by_name(input_port_name, "raumklang:rms_in")?;

    let dbfs = |v: f32| 20.0 * f32::log10(v.abs());
    loop {
        let iter = cons.pop_iter();
        let (_, count) = iter.size_hint();
        if let Some(count) = count {
            if count == 0 {
                continue;
            }
            let rms = iter.fold(0f32, |acc, val| acc + val * val) / count as f32;
            let rms = rms.sqrt();
            print!("\x1b[2K\rRMS: {} dBFS", dbfs(rms));
            io::stdout().flush().unwrap();
        }
    }
}

// legacy code
//pub fn compute_rir(record_path: &str, sweep_path: &str) -> anyhow::Result<()> {
//    let mut record_reader = hound::WavReader::open(record_path)?;
//    let mut sweep_reader = hound::WavReader::open(sweep_path)?;
//
//    println!("Samples of {}: {}", record_path, record_reader.duration());
//    println!("Samples of {}: {}", sweep_path, sweep_reader.duration());
//
//    let mut record_samples: Vec<f32> = record_reader
//        .samples::<f32>()
//        .collect::<Result<Vec<f32>, _>>()?;
//    let mut sweep_samples: Vec<f32> = sweep_reader
//        .samples::<f32>()
//        .collect::<Result<Vec<f32>, _>>()?;
//
//    let record_samples_count = record_samples.len();
//    let sweep_samples_count = sweep_samples.len();
//
//    println!("Samples of {}: {}", record_path, record_samples_count);
//    println!("Samples of {}: {}", sweep_path, sweep_samples_count);
//
//    // make record and sweep the same length
//    if record_samples_count > sweep_samples_count {
//        sweep_samples.append(&mut vec![0.0; record_samples_count - sweep_samples_count]);
//    } else {
//        record_samples.append(&mut vec![0.0; sweep_samples_count - record_samples_count]);
//    }
//
//    assert!(sweep_samples.len() == record_samples.len());
//
//    // double the size
//    record_samples.append(&mut vec![0.0; record_samples.len()]);
//    sweep_samples.append(&mut vec![0.0; sweep_samples.len()]);
//
//    // convert to complex
//    let mut record_samples: Vec<_> = record_samples.into_iter().map(Complex::from).collect();
//    let mut sweep_samples: Vec<_> = sweep_samples.into_iter().map(Complex::from).collect();
//
//    // convert into frequency domain
//    let mut planner = FftPlanner::<f32>::new();
//    let fft = planner.plan_fft_forward(record_samples.len());
//
//    fft.process(&mut record_samples);
//    fft.process(&mut sweep_samples);
//
//    // normalize
//    let scale: f32 = 1.0 / (record_samples.len() as f32).sqrt();
//    let record_samples: Vec<_> = record_samples.into_iter().map(|s| s * scale).collect();
//    let sweep_samples: Vec<_> = sweep_samples.into_iter().map(|s| s * scale).collect();
//
//    //plot_frequency_domain("recorded.png", &record_samples);
//    //plot_frequency_domain("sweep.png", &sweep_samples);
//
//    // devide both
//    let mut result: Vec<Complex<f32>> = record_samples
//        .iter()
//        .zip(sweep_samples.iter())
//        .map(|(r, s)| r / s)
//        .collect();
//
//    plot_frequency_domain("rir_fd.png", &result[..result.len() / 2]);
//
//    // back to time domain
//    let fft = planner.plan_fft_inverse(result.len());
//    fft.process(&mut result);
//
//    // normalize
//    let scale: f32 = 1.0 / (result.len() as f32).sqrt();
//    let result: Vec<_> = result.into_iter().map(|s| s * scale).collect();
//
//    plot_time_domain("rir_td.png", &result[..result.len() / 2]);
//
//    Ok(())
//}

//pub fn run_measurement(
//    input_device: &cpal::Device,
//    record_path: &str,
//    output_device: &cpal::Device,
//    sweep_path: &str,
//    duration: u8,
//) -> anyhow::Result<()> {
//    let input_config = input_device
//        .default_input_config()
//        .expect("Failed to get default input config");
//    println!("Default input config: {:?}", input_config);
//
//    let spec = wav_spec_from_config(&input_config);
//    let writer = hound::WavWriter::create(record_path, spec)?;
//    let writer = Arc::new(Mutex::new(Some(writer)));
//
//    // A flag to indicate that recording is in progress.
//    println!("Begin recording...");
//
//    // Run the input stream on a separate thread.
//    let writer_2 = writer.clone();
//
//    let err_fn = move |err| {
//        eprintln!("an error occurred on stream: {}", err);
//    };
//
//    let output_config = output_device
//        .default_input_config()
//        .expect("Failed to get default input config");
//    println!("Default output config: {:?}", output_config);
//
//    println!("Write sweep to file: {}", sweep_path);
//    let spec = wav_spec_from_config(&output_config);
//    let mut sweep_writer = hound::WavWriter::create(sweep_path, spec)?;
//
//    let sine_sweep = SineSweep::new(50, 5_000, duration.into(), 0.125, 44_100);
//    let sine_sweep: Vec<f32> = sine_sweep.into_iter().collect();
//
//    let stream = match input_config.sample_format() {
//        cpal::SampleFormat::F32 => input_device.build_input_stream(
//            &input_config.into(),
//            move |data, _: &_| write_input_data::<f32, f32>(data, &writer_2),
//            err_fn,
//            None,
//        )?,
//        cpal::SampleFormat::I16 => input_device.build_input_stream(
//            &input_config.into(),
//            move |data, _: &_| write_input_data::<i16, i16>(data, &writer_2),
//            err_fn,
//            None,
//        )?,
//        cpal::SampleFormat::U16 => input_device.build_input_stream(
//            &input_config.into(),
//            move |data, _: &_| write_input_data::<u16, i16>(data, &writer_2),
//            err_fn,
//            None,
//        )?,
//        _ => panic!("Sample format not supported!"),
//    };
//
//    stream.play()?;
//
//    match output_config.sample_format() {
//        cpal::SampleFormat::F32 => {
//            play_audio::<f32, _>(output_device, &output_config.into(), sine_sweep.clone())
//        }
//        cpal::SampleFormat::I16 => {
//            play_audio::<i16, _>(output_device, &output_config.into(), sine_sweep.clone())
//        }
//        cpal::SampleFormat::U16 => {
//            play_audio::<u16, _>(output_device, &output_config.into(), sine_sweep.clone())
//        }
//        _ => panic!("Sample format not supported!"),
//    }?;
//
//    drop(stream);
//    writer.lock().unwrap().take().unwrap().finalize()?;
//    println!("Recording {} complete!", record_path);
//
//    for sample in sine_sweep {
//        sweep_writer.write_sample(sample).ok();
//    }
//    sweep_writer.finalize().unwrap();
//    println!("Sweep file {} completed!", sweep_path);
//
//    Ok(())
//}
//
//type WavWriterHandle = Arc<Mutex<Option<hound::WavWriter<BufWriter<File>>>>>;
//fn write_input_data<T, U>(input: &[T], writer: &WavWriterHandle)
//where
//    T: cpal::Sample,
//    U: cpal::Sample + cpal::FromSample<T> + hound::Sample,
//{
//    // TODO: refactor
//    if let Ok(mut guard) = writer.try_lock() {
//        if let Some(writer) = guard.as_mut() {
//            // FIXME hardcode
//            for frame in input.chunks(2) {
//                for (channel, &sample) in frame.iter().enumerate() {
//                    // FIXME hardcode
//                    if channel == 0 {
//                        let sample = U::from_sample(sample);
//                        writer.write_sample(sample).ok();
//                    }
//                }
//            }
//        }
//    }
//}

//fn wav_spec_from_config(config: &cpal::SupportedStreamConfig) -> hound::WavSpec {
//    hound::WavSpec {
//        channels: 1,
//        sample_rate: config.sample_rate().0 as _,
//        bits_per_sample: (config.sample_format().sample_size() * 8) as _,
//        sample_format: sample_format(config.sample_format()),
//    }
//}
//
//fn sample_format(format: cpal::SampleFormat) -> hound::SampleFormat {
//    match format {
//        cpal::SampleFormat::U16 => hound::SampleFormat::Int,
//        cpal::SampleFormat::I16 => hound::SampleFormat::Int,
//        cpal::SampleFormat::F32 => hound::SampleFormat::Float,
//        _ => panic!("Sample format not supported!"),
//    }
//}

//pub fn old_rir(
//    host: &cpal::Host,
//    device: &cpal::Device,
//    config: cpal::SupportedStreamConfig,
//) -> anyhow::Result<()> {
//    let duration = 10;
//    let input_device = host.default_input_device().unwrap();
//
//    let input_config = input_device
//        .default_input_config()
//        .expect("Failed to get default input config");
//    println!("Default input config: {:?}", input_config);
//
//    let input_stream_config: cpal::StreamConfig = input_config.clone().into();
//
//    let writer: Vec<f32> =
//        Vec::with_capacity(duration * input_stream_config.sample_rate.0 as usize);
//    let writer = Arc::new(Mutex::new(Some(writer)));
//
//    // A flag to indicate that recording is in progress.
//    println!("Begin recording...");
//
//    // Run the input stream on a separate thread.
//    let writer_2 = writer.clone();
//
//    let err_fn = move |err| {
//        eprintln!("an error occurred on stream: {}", err);
//    };
//
//    let stream = match input_config.sample_format() {
//        cpal::SampleFormat::F32 => device.build_input_stream(
//            &input_config.into(),
//            move |data, _: &_| write_input_data_ram::<f32>(data, &writer_2),
//            err_fn,
//            None,
//        )?,
//        sample_format => {
//            return Err(anyhow::Error::msg(format!(
//                "Unsupported sample format '{:?}'",
//                sample_format
//            )))
//        }
//    };
//
//    stream.play()?;
//
//    //Sweep
//    let start_frequency = 50;
//    let end_frequency = 5_000;
//    let gain = 0.3;
//    let sample_rate = input_stream_config.sample_rate.0;
//    let sine_sweep = SineSweep::new(
//        start_frequency,
//        end_frequency,
//        duration as u32,
//        gain,
//        sample_rate,
//    );
//
//    let sine_sweep: Vec<f32> = sine_sweep.collect();
//    let sine_sweep_clone: Vec<f32> = sine_sweep.clone();
//
//    match config.sample_format() {
//        cpal::SampleFormat::F32 => play_audio::<f32, _>(device, &config.into(), sine_sweep),
//        cpal::SampleFormat::I16 => play_audio::<i16, _>(device, &config.into(), sine_sweep),
//        cpal::SampleFormat::U16 => play_audio::<u16, _>(device, &config.into(), sine_sweep),
//        _ => panic!("Sample format not supported!"),
//    }?;
//
//    drop(stream);
//    println!("Recording complete!");
//
//    let mut guard = writer.lock().unwrap();
//    let recording = guard.take().unwrap();
//    // convert to complex numbers
//    let mut recording: Vec<Complex<f32>> = recording.into_iter().map(Complex::from).collect();
//    // double size and fill with 0
//    plot_time_domain("recording.png", &recording);
//
//    recording.append(&mut vec![Complex::from(0.0); recording.len()]);
//    convert_to_frequency_domain(&mut recording);
//
//    // Sweep signal
//    let mut sweep_complex: Vec<Complex<f32>> =
//        sine_sweep_clone.into_iter().map(Complex::from).collect();
//    sweep_complex.append(&mut vec![Complex::from(0.0); recording.len()]);
//    plot_time_domain("sweep.png", &sweep_complex);
//    convert_to_frequency_domain(&mut sweep_complex);
//
//    let mut result: Vec<Complex<f32>> = recording
//        .iter()
//        .zip(sweep_complex.iter())
//        .map(|(r, s)| r / s)
//        .collect();
//
//    let scale = Complex::from(1.0 / (result.len() as f32 / 2.0).sqrt());
//    let result_scaled: Vec<Complex<f32>> = result.iter().map(|&i| i * scale).collect();
//    let ref_point: f32 = result_scaled.iter().map(|&r| r.re).sum();
//    let result_scaled: Vec<Complex<f32>> = result_scaled
//        .iter()
//        .map(|&y| Complex::from(20. * f32::log10(2. * y.re / ref_point)))
//        .collect();
//    plot_frequency_domain("rir_fd.png", &result_scaled);
//
//    let mut planner = FftPlanner::<f32>::new();
//    let fft = planner.plan_fft_inverse(result.len() / 2);
//
//    fft.process(&mut result);
//
//    // normalize
//    let scale = Complex::from(1.0 / (result.len() as f32 / 2.0));
//    let result: Vec<Complex<f32>> = result.iter_mut().map(|&mut i| i * scale).collect();
//
//    let result_real: Vec<_> = result.iter().map(|y| y.re).collect();
//
//    draw_spectrogram("rir_spec.png", &result_real);
//
//    plot_frequency_domain("rir.png", &result[0..result.len() / 2]);
//
//    Ok(())
//}

//pub fn plot_fake_impulse_respons() -> anyhow::Result<()> {
//    let sine_sweep = SineSweep::new(50, 10000, 10, 0.8, 44_100);
//    let mut buffer: Vec<Complex<f32>> = sine_sweep.map(Complex::from).collect();
//    buffer.append(&mut vec![Complex::from(0.0); buffer.len()]);
//
//    convert_to_frequency_domain(&mut buffer);
//    plot_frequency_domain("sweep_fd.png", &buffer);
//
//    let recorded_signal = buffer.clone();
//    let mut result: Vec<Complex<f32>> = recorded_signal
//        .iter()
//        .zip(buffer.iter())
//        .map(|(r, s)| r / s)
//        .collect();
//
//    plot_frequency_domain("div_fd.png", &buffer);
//
//    let mut planner = FftPlanner::<f32>::new();
//    let fft = planner.plan_fft_inverse(result.len() / 2);
//
//    fft.process(&mut result);
//
//    plot_time_domain("ir_td.png", &result);
//    Ok(())
//}

//pub fn ping_pong(host: &cpal::Host, device: &OutputDevice) -> anyhow::Result<()> {
//    // Set up the input device and stream with the default input config.
//    let input_device = host.default_input_device().unwrap();
//    //} else {
//    //    host.input_devices()?
//    //        .find(|x| x.name().map(|y| y == opt.device).unwrap_or(false))
//    //}
//    //.expect("failed to find input device")
//    let data = start_record(&input_device, 10).unwrap();
//    println!("Data length: {}", data.len());
//    //let data: Vec<Complex<f32>> = data.into_iter().map(|m| Complex::from(m)).collect();
//
//    //match config.sample_format() {
//    //    cpal::SampleFormat::F32 => run::<f32, _>(device, &config.into(), data),
//    //    cpal::SampleFormat::I16 => run::<i16, _>(device, &config.into(), data),
//    //    cpal::SampleFormat::U16 => run::<u16, _>(device, &config.into(), data),
//    //}
//    device.play::<f32, _>(data, None)
//}
//
//pub fn convert_to_frequency_domain(buffer: &mut Vec<Complex<f32>>) {
//    //let fill_len = 1024 - buffer.len() % 1024;
//    //buffer.append(&mut vec![Complex::from(0.0); fill_len]);
//
//    let mut planner = FftPlanner::<f32>::new();
//    let fft = planner.plan_fft_forward(buffer.len() / 2);
//
//    fft.process(buffer);
//}
//
//use plotters::coord::combinators::IntoLogRange;
//
//pub fn plot_frequency_domain(file_name: &str, buffer: &[Complex<f32>]) {
//    let root_drawing_area = BitMapBackend::new(file_name, (6000, 768)).into_drawing_area();
//    root_drawing_area.fill(&WHITE).unwrap();
//    //let x_cord: LogCoord<f32> = (0.0..6000.0).log_scale().into();
//    //let max_freq = (buffer.len() / 2 - 1) as f32 * 44_100.0 / buffer.len() as f32;
//    let max_freq: f32 = 5_000.0;
//    //let values: Vec<(_, _)> = buffer.iter().enumerate().collect();
//    let dbfs = |v: f32| 20.0 * f32::log10(v.abs());
//    let buf_size = buffer.len() * 2;
//    let upper_bound = (max_freq * buffer.len() as f32 * 2.0 / 44100.0) as usize;
//    let values: Vec<(_, _)> = buffer[0..upper_bound]
//        .iter()
//        .enumerate()
//        .map(|(n, y)| (n as f32 * 44_100.0 / buf_size as f32, dbfs(y.norm())))
//        .map(|(x, y)| {
//            if y == f32::NEG_INFINITY {
//                (x, 0f32)
//            } else {
//                (x, y)
//            }
//        })
//        .collect();
//
//    let min = values
//        .iter()
//        .map(|(_, y)| y)
//        .fold(0f32, |min, &val| if val < min { val } else { min });
//
//    let max = values
//        .iter()
//        .map(|(_, y)| y)
//        .fold(0f32, |max, &val| if val > max { val } else { max });
//    println!("Min: {:?}, Max: {:?}", min, max);
//    //let values: Vec<(f32, f32)> = values[50..5000].iter().map(|(n, y)| (*n as f32, y.re)).collect();
//
//    //let values = values[0..max_freq as usize].to_vec();
//    let mut chart = ChartBuilder::on(&root_drawing_area)
//        .set_label_area_size(LabelAreaPosition::Left, 60)
//        .set_label_area_size(LabelAreaPosition::Bottom, 60)
//        .build_cartesian_2d(0.0..max_freq, (-80.0f32..0.0f32).log_scale())
//        .unwrap();
//
//    chart
//        .configure_mesh()
//        .x_labels(60)
//        .y_labels(10)
//        .disable_mesh()
//        .x_label_formatter(&|v| format!("{}", v))
//        .y_label_formatter(&|v| format!("{}", v))
//        .draw()
//        .unwrap();
//
//    chart.draw_series(LineSeries::new(values, &RED)).unwrap();
//    //chart.draw_series(LineSeries::new(
//    //    sine_sweep.enumerate().map(|(n, y)| {
//    //        //let overall_samples = 10.0 * 44100.0;
//    //        //((n as f32 / overall_samples), y)
//    //        (n as f32, y)
//    //    }),
//    //    &RED
//    //)).unwrap();
//}
//
//fn plot_time_domain(file_name: &str, buffer: &[Complex<f32>]) {
//    let root_drawing_area = BitMapBackend::new(file_name, (6000, 768)).into_drawing_area();
//    root_drawing_area.fill(&WHITE).unwrap();
//
//    let max_time: f32 = 0.5;
//    let last_sample = (max_time * 44100.0) as usize;
//    //let x_cord: LogCoord<f32> = (0.0..6000.0).log_scale().into();
//    let mut chart = ChartBuilder::on(&root_drawing_area)
//        .set_label_area_size(LabelAreaPosition::Left, 60)
//        .set_label_area_size(LabelAreaPosition::Bottom, 60)
//        .build_cartesian_2d(0.0..max_time, -1.0..1.0f32)
//        .unwrap();
//
//    //let values: Vec<(_, _)> = buffer.iter().enumerate().collect();
//    let values: Vec<(_, _)> = buffer[0..last_sample]
//        .iter()
//        .enumerate()
//        .map(|(n, y)| (n as f32 / 44100.0, y.re))
//        .collect();
//    //let values: Vec<(f32, f32)> = values[50..5000].iter().map(|(n, y)| (*n as f32, y.re)).collect();
//
//    chart
//        .configure_mesh()
//        .x_labels(20)
//        .y_labels(10)
//        .disable_mesh()
//        .x_label_formatter(&|v| format!("{}", v))
//        .y_label_formatter(&|v| format!("{:.1}", v))
//        .draw()
//        .unwrap();
//
//    chart.draw_series(LineSeries::new(values, &RED)).unwrap();
//    //chart.draw_series(LineSeries::new(
//    //    sine_sweep.enumerate().map(|(n, y)| {
//    //        //let overall_samples = 10.0 * 44100.0;
//    //        //((n as f32 / overall_samples), y)
//    //        (n as f32, y)
//    //    }),
//    //    &RED
//    //)).unwrap();
//}

//pub fn start_record(device: &cpal::Device, duration: usize) -> Result<Vec<f32>, anyhow::Error> {
//    let config = device
//        .default_input_config()
//        .expect("Failed to get default input config");
//    println!("Default input config: {:?}", config);
//
//    let stream_config: cpal::StreamConfig = config.clone().into();
//
//    let writer: Vec<f32> = Vec::with_capacity(duration * stream_config.sample_rate.0 as usize);
//    let writer = Arc::new(Mutex::new(Some(writer)));
//
//    // A flag to indicate that recording is in progress.
//    println!("Begin recording...");
//
//    // Run the input stream on a separate thread.
//    let writer_2 = writer.clone();
//
//    let err_fn = move |err| {
//        eprintln!("an error occurred on stream: {}", err);
//    };
//
//    let stream = match config.sample_format() {
//        cpal::SampleFormat::F32 => device.build_input_stream(
//            &config.into(),
//            move |data, _: &_| write_input_data_ram::<f32>(data, &writer_2),
//            err_fn,
//            None,
//        )?,
//        sample_format => {
//            return Err(anyhow::Error::msg(format!(
//                "Unsupported sample format '{:?}'",
//                sample_format
//            )))
//        }
//    };
//
//    stream.play()?;
//
//    // Let recording go for roughly three seconds.
//    std::thread::sleep(std::time::Duration::from_secs(duration as u64));
//    drop(stream);
//    println!("Recording complete!");
//
//    let mut guard = writer.lock().unwrap();
//    Ok(guard.take().unwrap())
//}
//
//fn write_input_data_ram<T>(input: &[T], writer: &Arc<Mutex<Option<Vec<T>>>>)
//where
//    T: Sample,
//{
//    if let Ok(mut guard) = writer.try_lock() {
//        if let Some(data) = guard.as_mut() {
//            for frame in input.chunks(2) {
//                for (channel, &sample) in frame.iter().enumerate() {
//                    if channel == 0 {
//                        data.push(sample);
//                    }
//                }
//            }
//        }
//    }
//}
//
//const WINDOW_SIZE: usize = 1024;
//const OVERLAP: f64 = 0.9;
//const SKIP_SIZE: usize = (WINDOW_SIZE as f64 * (1f64 - OVERLAP)) as usize;
//
//fn draw_spectrogram(file_name: &str, samples: &[f32]) {
//    //let sine_sweep = SineSweep::new(50, 15000, 10, 1.0, 44100);
//    //let samples: Vec<f32> = sine_sweep.collect();
//
//    println!("Creating windows {window_size} samples long from a timeline {num_samples} samples long, picking every {skip_size} windows with a {overlap} overlap for a total of {num_windows} windows.",
//        window_size = WINDOW_SIZE, num_samples = samples.len(), skip_size = SKIP_SIZE, overlap = OVERLAP, num_windows = (samples.len() / SKIP_SIZE) - 1,
//    );
//
//    // Convert to an ndarray
//    // Hopefully this will keep me from messing up the dimensions
//    // Mutable because the FFT takes mutable slices &[Complex<f32>]
//    // let window_array = Array2::from_shape_vec((WINDOW_SIZE, windows_vec.len()), windows_vec).unwrap();
//
//    let samples_array = Array::from(samples.to_owned());
//    let windows = samples_array
//        .windows(ndarray::Dim(WINDOW_SIZE))
//        .into_iter()
//        .step_by(SKIP_SIZE)
//        .collect::<Vec<_>>();
//    let windows = ndarray::stack(Axis(0), &windows).unwrap();
//
//    // So to perform the FFT on each window we need a Complex<f32>, and right now we have i16s, so first let's convert
//    let mut windows = windows.map(|i| Complex::from(*i));
//
//    // get the FFT up and running
//    let mut planner = FftPlanner::new();
//    let fft = planner.plan_fft_forward(WINDOW_SIZE);
//
//    // Since we have a 2-D array of our windows with shape [WINDOW_SIZE, (num_samples / WINDOW_SIZE) - 1], we can run an FFT on every row.
//    // Next step is to do something multithreaded with Rayon, but we're not cool enough for that yet.
//    windows.axis_iter_mut(Axis(0)).for_each(|mut frame| {
//        fft.process(frame.as_slice_mut().unwrap());
//    });
//
//    // Get the real component of those complex numbers we get back from the FFT
//    let windows = windows.map(|i| i.re);
//
//    // And finally, only look at the first half of the spectrogram - the first (n/2)+1 points of each FFT
//    // https://dsp.stackexchange.com/questions/4825/why-is-the-fft-mirrored
//    let windows = windows.slice_move(ndarray::s![.., ..((WINDOW_SIZE / 2) + 1)]);
//
//    // get some dimensions for drawing
//    // The shape is in [nrows, ncols], but we want to transpose this.
//    let (width, height) = match windows.shape() {
//        &[first, second] => (first, second),
//        _ => panic!(
//            "Windows is a {}D array, expected a 2D array",
//            windows.ndim()
//        ),
//    };
//
//    println!("Generating a {} wide x {} high image", width, height);
//
//    let image_dimensions: (u32, u32) = (width as u32, height as u32);
//    let root_drawing_area = BitMapBackend::new(
//        file_name,
//        image_dimensions, // width x height. Worth it if we ever want to resize the graph.
//    )
//    .into_drawing_area();
//
//    let spectrogram_cells = root_drawing_area.split_evenly((height, width));
//
//    let windows_scaled = windows.map(|i| i.abs() / (WINDOW_SIZE as f32));
//    let highest_spectral_density = windows_scaled.max_skipnan();
//
//    // transpose and flip around to prepare for graphing
//    /* the array is currently oriented like this:
//        t = 0 |
//              |
//              |
//              |
//              |
//        t = n +-------------------
//            f = 0              f = m
//
//        so it needs to be flipped...
//        t = 0 |
//              |
//              |
//              |
//              |
//        t = n +-------------------
//            f = m              f = 0
//
//        ...and transposed...
//        f = m |
//              |
//              |
//              |
//              |
//        f = 0 +-------------------
//            t = 0              t = n
//
//        ... in order to look like a proper spectrogram
//    */
//    let windows_flipped = windows_scaled.slice(ndarray::s![.., ..; -1]); // flips the
//    let windows_flipped = windows_flipped.t();
//
//    // Finally add a color scale
//    let color_scale = colorous::MAGMA;
//
//    for (cell, spectral_density) in spectrogram_cells.iter().zip(windows_flipped.iter()) {
//        let spectral_density_scaled = spectral_density.sqrt() / highest_spectral_density.sqrt();
//        let color = color_scale.eval_continuous(spectral_density_scaled as f64);
//        cell.fill(&RGBColor(color.r, color.g, color.b)).unwrap();
//    }
//}
