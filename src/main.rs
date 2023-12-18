use std::{
    io::{self, Write},
    sync::mpsc::Receiver,
    time::{Duration, Instant},
};

use clap::{Parser, Subcommand};

use raumklang::{
    volume_to_amplitude, AudioEngine, FiniteSignal, LinearSineSweep, PinkNoise, WhiteNoise,
};
use ringbuf::{HeapRb, Rb};

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
    Plot,
    PingPong,
    Rir,
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
    ComputeRIR,
    Rms {
        #[arg(short, long)]
        input_port: String,
    },
}

#[derive(Subcommand)]
enum SignalType {
    WhiteNoise,
    PinkNoise,
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
            let mut writer = hound::WavWriter::create(file_path, spec)?;
            loop {
                if let Some(s) = buf.pop() {
                    writer.write_sample(s)?;
                }

                if repsose.try_recv().is_ok() {
                    break;
                }
            }
            writer.finalize()?;

            Ok(())
        }
        _ => panic!("Not implemented!"),
    }
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
        SignalType::LogSweep {
            start_frequency,
            end_frequency,
        } => {
            let sweep = LinearSineSweep::new(
                start_frequency,
                end_frequency,
                duration,
                amplitude,
                sample_rate,
            );
            println!("{}", sweep.len());
            Box::new(sweep)
        }
    };

    engine.play_signal(signal)
}

pub fn meter_rms(source_port_name: &str) -> anyhow::Result<()> {
    let jack_client_name = env!("CARGO_BIN_NAME");

    let engine = AudioEngine::new(jack_client_name)?;

    // FIXME: type problem
    engine.play_signal([0.0])?;

    let mut cons = engine.register_in_port("rms_in", source_port_name)?;

    let mut last_rms = Instant::now();
    let mut last_peak = Instant::now();
    let mut peak = f32::NEG_INFINITY;

    let dbfs = |v: f32| 20.0 * f32::log10(v);

    let window_size = 147;
    let mut window = HeapRb::<_>::new(window_size);
    let mut sum_sq = 0f32;

    loop {
        let iter = cons.pop_iter();

        for s in iter {
            let s_sq = s.powi(2);
            sum_sq += s_sq;

            peak = peak.max(s);

            let removed = window.push_overwrite(s_sq);
            if let Some(r_sq) = removed {
                sum_sq -= r_sq;
            }
        }

        if last_rms.elapsed() > Duration::from_millis(200) {
            print!(
                "\x1b[2K\rRMS: {:>8.2} dBFS, Peak: {:>8.2} dbFS",
                dbfs((sum_sq / window_size as f32).sqrt()),
                dbfs(peak)
            );
            io::stdout().flush().unwrap();

            last_rms = Instant::now();
        }

        if last_peak.elapsed() > Duration::from_millis(500) {
            peak = dbfs((sum_sq / window_size as f32).sqrt());

            window.clear();
            sum_sq = 0f32;

            last_peak = Instant::now();
        }

        std::thread::sleep(Duration::from_millis(150));
    }
}
