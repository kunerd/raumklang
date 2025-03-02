use std::{
    io::{self, Write},
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use clap::{Parser, Subcommand};

use ndarray::{Array, Axis};
use ndarray_stats::QuantileExt;
use plotters::{
    prelude::{BitMapBackend, IntoDrawingArea},
    style::RGBColor,
};
use raumklang_core::{
    dbfs, loudness,
    signals::{ExponentialSweep, FiniteSignal, LinearSineSweep, PinkNoise, WhiteNoise},
    volume_to_amplitude, AudioEngine, ImpulseResponse,
};
use rustfft::{num_complex::Complex, FftPlanner};

#[derive(Parser)]
#[clap(author, version)]
struct Cli {
    #[clap(long)]
    plot: bool,
    #[command(subcommand)]
    subcommand: Command,
}

#[derive(Subcommand)]
enum Command {
    Rms {
        #[arg(short, long)]
        input_port: String,
    },
    Signal {
        #[clap(short, long, default_value_t = 5)]
        duration: usize,
        #[clap(short, long, default_value_t = 0.5)]
        volume: f32,
        #[arg(long = "dest-port")]
        dest_ports: Vec<String>,
        #[arg(long)]
        file_path: Option<String>,
        #[command(subcommand)]
        type_: SignalType,
    },
    RunMeasurement {
        #[clap(short, long, default_value_t = 5)]
        duration: usize,
        #[clap(short, long, default_value_t = 0.5)]
        volume: f32,
        #[arg(long = "dest-port")]
        dest_ports: Vec<String>,
        #[arg(short, long)]
        input_port: String,
        #[arg(long)]
        file_path: String,
        #[command(subcommand)]
        type_: SignalType,
    },
    ComputeRIR {
        loopback_path: String,
        measurement_path: String,
        result_path: String,
    },
    Spectrogram {
        file_path: String,
    },
}

#[derive(Subcommand)]
enum SignalType {
    WhiteNoise,
    PinkNoise,
    LinearSweep {
        #[clap(short, long, default_value_t = 50)]
        start_frequency: u16,
        #[clap(short, long, default_value_t = 1000)]
        end_frequency: u16,
    },
    LogSweep {
        #[clap(short, long, default_value_t = 50)]
        start_frequency: u16,
        #[clap(short, long, default_value_t = 1000)]
        end_frequency: u16,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.subcommand {
        Command::Signal {
            duration,
            volume,
            dest_ports,
            file_path: _,
            type_,
        } => {
            let engine = init_playback_engine(&dest_ports)?;
            let response = play_signal(&engine, type_, volume, duration)?;
            response.recv()?;
            Ok(())
        }
        Command::Rms { input_port } => meter_rms(&input_port),
        Command::RunMeasurement {
            duration,
            volume,
            dest_ports,
            input_port,
            type_,
            file_path,
        } => {
            let engine = init_playback_engine(&dest_ports)?;
            let mut buf = engine.register_in_port("measurement_in", &input_port)?;
            let repsose = play_signal(&engine, type_, volume, duration)?;

            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: engine.sample_rate() as u32,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };

            // FIXME hardcoded window size
            let mut loudness = loudness::Meter::new(13230); // 44100samples / 1000ms * 300ms
            let mut writer = hound::WavWriter::create(file_path, spec)?;
            loop {
                let iter = buf.pop_iter();
                for s in iter {
                    loudness.update(s);
                    writer.write_sample(s)?;
                }

                if repsose.try_recv().is_ok() {
                    break;
                }

                std::thread::sleep(Duration::from_millis(10)); // buf size is 1024, 1 / 44100 *
                                                               // 1024 = 0,023 s = 23ms / 2 = 11,5
                                                               //      ~ 10
            }
            writer.finalize()?;
            println!(
                "rms: {} dbfs, peak: {} dbfs",
                dbfs(loudness.rms()),
                dbfs(loudness.peak())
            );

            Ok(())
        }
        Command::ComputeRIR {
            loopback_path,
            measurement_path,
            result_path,
        } => {
            let impulse_respone = ImpulseResponse::from_files(&loopback_path, &measurement_path)?;

            let spec = hound::WavSpec {
                channels: 1,
                sample_rate: impulse_respone.sample_rate,
                bits_per_sample: 32,
                sample_format: hound::SampleFormat::Float,
            };

            let mut writer = hound::WavWriter::create(&result_path, spec)?;
            for s in impulse_respone.data.iter().map(|s| s.re) {
                writer.write_sample(s)?;
            }
            writer.finalize()?;

            let duration = impulse_respone.data.len() as f32 / impulse_respone.sample_rate as f32;
            println!("Impulse response of : {duration}s, written to: {result_path}");

            Ok(())
        }
        Command::Spectrogram { file_path } => {
            let mut reader = hound::WavReader::open(file_path)?;
            let data: Vec<f32> = reader.samples::<f32>().collect::<Result<Vec<f32>, _>>()?;

            let data: Vec<_> = data.iter().map(Complex::from).collect();

            plot_heatmap(data)?;

            Ok(())
        }
    }
}

fn plot_heatmap(ir: Vec<Complex<f32>>) -> anyhow::Result<()> {
    let _sample_rate = 44100;
    let window_size = 4 * 1024;

    //window = np.append(
    //        signal.get_window("hann", window_size)[0: window_size //2],
    //        signal.get_window("tukey", window_size, .25)[window_size//2:]
    //)
    //#window = signal.get_window("hamming", window_size)
    //
    let start_sample = 0;
    let stop_sample = ir.len();
    //
    let time_shift = 15; // ms
    let time_shift_samples = 44100 * time_shift / 1000;
    let rem = (stop_sample - start_sample - window_size) % time_shift_samples;
    let _stop_sample = stop_sample + time_shift - rem;

    //let window_count = stop_sample - start_sample - window_size / time_shift_samples;
    //let mut planner = FftPlanner::<f32>::new();
    //let fft = planner.plan_fft_forward(window_size * 2);

    //let mut ffts = vec![];
    //for n in 0..window_count {
    //    let start = n * time_shift_samples;
    //    let end = start + window_size;

    //    let mut chunk = Vec::with_capacity(window_size);
    //    chunk.clone_from_slice(&ir[start..end]);

    //    fft.process(&mut chunk);

    //    ffts.push(chunk)
    //}

    let samples_array = Array::from(ir.clone());

    const MAX_FREQ: usize = 1000;
    const WINDOW_SIZE: usize = 44100 * 300 / 1000;
    const OVERLAP: f64 = 0.9;
    const SKIP_SIZE: usize = (WINDOW_SIZE as f64 * (1f64 - OVERLAP)) as usize;

    let windows = samples_array
        .windows(ndarray::Dim(WINDOW_SIZE))
        .into_iter()
        .step_by(SKIP_SIZE)
        .collect::<Vec<_>>();
    let mut windows = ndarray::stack(Axis(0), &windows).unwrap();

    // So to perform the FFT on each window we need a Complex<f32>, and right now we have i16s, so first let's convert
    //let mut windows = windows.map(|i| Complex::from(*i));

    // get the FFT up and running
    let mut planner = FftPlanner::new();
    let fft = planner.plan_fft_forward(WINDOW_SIZE);

    // Since we have a 2-D array of our windows with shape [WINDOW_SIZE, (num_samples / WINDOW_SIZE) - 1], we can run an FFT on every row.
    // Next step is to do something multithreaded with Rayon, but we're not cool enough for that yet.
    windows.axis_iter_mut(Axis(0)).for_each(|mut frame| {
        fft.process(frame.as_slice_mut().unwrap());
    });

    // Get the real component of those complex numbers we get back from the FFT
    //let windows = windows.map(|i| i.re);
    let windows = windows.map(|i| i.norm());

    // And finally, only look at the first half of the spectrogram - the first (n/2)+1 points of each FFT
    // https://dsp.stackexchange.com/questions/4825/why-is-the-fft-mirrored
    //let windows = windows.slice_move(ndarray::s![.., ..(WINDOW_SIZE / 2) + 1]);
    let windows = windows.slice_move(ndarray::s![.., ..MAX_FREQ * WINDOW_SIZE / 44100 + 1]);

    // get some dimensions for drawing
    // The shape is in [nrows, ncols], but we want to transpose this.
    let (width, height) = match windows.shape() {
        &[first, second] => (first, second),
        _ => panic!(
            "Windows is a {}D array, expected a 2D array",
            windows.ndim()
        ),
    };

    println!("Generating a {} wide x {} high image", width, height);

    let image_dimensions: (u32, u32) = (width as u32, height as u32);
    let root_drawing_area = BitMapBackend::new(
        "data/output.png",
        image_dimensions, // width x height. Worth it if we ever want to resize the graph.
    )
    .into_drawing_area();

    let spectrogram_cells = root_drawing_area.split_evenly((height, width));

    //let scale: f32 = 1.0 / (WINDOW_SIZE as f32).sqrt();
    //let windows_scaled = windows.map(|i| i.abs() * scale);
    let windows_scaled = windows.map(|i| i.abs());

    let highest_spectral_density = windows_scaled.max_skipnan();

    // transpose and flip around to prepare for graphing
    /* the array is currently oriented like this:
        t = 0 |
              |
              |
              |
              |
        t = n +-------------------
            f = 0              f = m

        so it needs to be flipped...
        t = 0 |
              |
              |
              |
              |
        t = n +-------------------
            f = m              f = 0

        ...and transposed...
        f = m |
              |
              |
              |
              |
        f = 0 +-------------------
            t = 0              t = n

        ... in order to look like a proper spectrogram
    */
    let windows_flipped = windows_scaled.slice(ndarray::s![.., ..; -1]); // flips the
    let windows_flipped = windows_flipped.t();

    // Finally add a color scale
    //let color_scale = colorous::MAGMA;
    let color_scale = colorous::TURBO;

    for (cell, spectral_density) in spectrogram_cells.iter().zip(windows_flipped.iter()) {
        let spectral_density_scaled = spectral_density.sqrt() / highest_spectral_density.sqrt();
        //let spectral_density_scaled = spectral_density / highest_spectral_density;

        //let sd_log = spectral_density_scaled.log10();

        let color = color_scale.eval_continuous(spectral_density_scaled as f64);
        cell.fill(&RGBColor(color.r, color.g, color.b)).unwrap();
    }

    Ok(())
    //freq_time = np.zeros(ffts[0].shape, dtype="complex")
    //for win_fft in ffts:
    //    freq_time += win_fft ** 2
    //
    //#plot_frequency_domain(np.abs(np.sqrt(freq_time)))
    //
    //fr = 44100 / 2 / window_size
    //freqs = np.arange(0, window_size // 2 + 1) * fr
    //
    //print(len(ffts))
    //db_fs_spec = 20 * np.log10(np.abs(ffts))
    //#db_fs_spec = np.abs(ffts)
    //
    //#ticks = np.array([0, 100, 500, 1000, 2000, 5000])
    //#ha.xaxis.set_ticks(ticks)
    //#ha.xaxis.set_ticklabels(ticks)
    //t = np.arange(0, len(ffts)) * time_shift
    //
    //#    ha = plt.subplot()
    //#    ha.set_xscale('log')
    //#    #cb = ha.pcolormesh(freqs, t, db_fs_spec, vmin=-60.0, vmax=-20, shading='gouraud')
    //#    cb = ha.pcolormesh(freqs, t, db_fs_spec, shading='gouraud')
    //#    #cb = ha.pcolormesh(freqs, range(0, len(ffts)), db_fs_spec, shading='gouraud')
    //#    plt.colorbar(cb)
    //#    plt.xlabel('Frequency [Hz]')
    //#    plt.ylabel('Time [ms]')
    //#    plt.xlim([50, 5000])
    //#    #plt.ylim([0, 0.6])
    //#    #plt.colorbar()
    //#    plt.show()
    //#    #plt.plot(freq_time)
    //#    #plt.show()
    //import plotly.graph_objects as go
    //
    //# Creating 2-D grid of features
    //#[X, Y] = np.meshgrid(feature_x, feature_y)
    //#
    //#Z = np.cos(X / 2) + np.sin(Y / 4)
    //
    //#fig = go.Figure(data =
    //#                go.Heatmap(x = freqs, y = t, z = db_fs_spec, zmin=-70, zmax=0))
    //#
    //#fig.show()
    //
    //fig = go.Figure(data=[go.Surface(
    //    z=db_fs_spec,
    //    x=freqs.flatten(),
    //    y=t.flatten()
    //)])
    //fig.update_layout(title='Mt Bruno Elevation',
    //                  #autosize=False,
    //                  #width=500, height=500,
    //                  #margin=dict(l=65, r=50, b=65, t=90)
    //)
    //fig.show()
}

fn init_playback_engine<T, I, J>(dest_ports: &[T]) -> anyhow::Result<AudioEngine<I, J>>
where
    T: AsRef<str>,
    I: Iterator<Item = f32> + Send + 'static,
    J: IntoIterator<IntoIter = I> + Send + Sync + 'static,
{
    let jack_client_name = env!("CARGO_BIN_NAME");
    let engine = AudioEngine::new(jack_client_name)?;
    engine.register_out_port("signal_out", dest_ports)?;

    Ok(engine)
}

fn play_signal(
    engine: &AudioEngine<Box<dyn FiniteSignal<Item = f32>>, Box<dyn FiniteSignal<Item = f32>>>,
    type_: SignalType,
    volume: f32,
    duration: usize,
) -> anyhow::Result<Receiver<bool>> {
    let sample_rate = engine.sample_rate();
    let amplitude = volume_to_amplitude(volume);

    let signal: Box<dyn FiniteSignal<Item = f32>> = match type_ {
        SignalType::WhiteNoise => {
            Box::new(WhiteNoise::with_amplitude(amplitude).take_duration(sample_rate, duration))
        }
        SignalType::PinkNoise => {
            Box::new(PinkNoise::with_amplitude(amplitude).take_duration(sample_rate, duration))
        }
        SignalType::LinearSweep {
            start_frequency,
            end_frequency,
        } => {
            let duration = Duration::from_secs(duration as u64);
            let sweep = LinearSineSweep::new(
                start_frequency,
                end_frequency,
                duration,
                amplitude,
                sample_rate,
            );
            Box::new(sweep)
        }
        SignalType::LogSweep {
            start_frequency,
            end_frequency,
        } => {
            let n_samples = duration * sample_rate;
            let sweep = ExponentialSweep::new(
                start_frequency as f32,
                end_frequency as f32,
                amplitude,
                n_samples,
                sample_rate,
            );

            // Box::new(sweep.iter().collect::<Vec<_>>().into_iter())
            Box::new(sweep.into_iter())
        }
    };

    Ok(engine.play_signal(signal)?)
}

pub fn meter_rms(source_port_name: &str) -> anyhow::Result<()> {
    let jack_client_name = env!("CARGO_BIN_NAME");

    let engine = AudioEngine::new(jack_client_name)?;

    // FIXME: type problem
    engine.play_signal([0.0])?;

    let mut cons = engine.register_in_port("rms_in", source_port_name)?;

    let mut last_rms = Instant::now();
    let mut last_peak = Instant::now();

    // FIXME hardcoded window size
    let mut loudness = loudness::Meter::new(13230); // 44100samples / 1000ms * 300ms

    loop {
        let iter = cons.pop_iter();
        if loudness.update_from_iter(iter) {
            last_peak = Instant::now();
        }

        if last_rms.elapsed() > Duration::from_millis(150) {
            print!(
                "\x1b[2K\rRMS: {:>8.2} dBFS, Peak: {:>8.2} dbFS",
                dbfs(loudness.rms()),
                dbfs(loudness.peak())
            );
            io::stdout().flush().unwrap();

            last_rms = Instant::now();
        }

        if last_peak.elapsed() > Duration::from_millis(500) {
            loudness.reset_peak();
            last_peak = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(75));
    }
}
