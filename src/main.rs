use std::path;

use clap::{Parser, Subcommand};

use raumklang::{
    meter_rms, play_signal, run_measurement, write_signal_to_file, FiniteSignal,
    LinearSineSweep, PinkNoise, PlaySignalConfig, WhiteNoise,
};

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
        duration: u8,
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
        duration: u8,
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

    match &cli.subcommand {
        Command::Signal {
            duration,
            volume,
            dest_ports,
            file_path,
            type_,
        } => {
            let config = PlaySignalConfig {
                out_port_name: "signal_out",
                dest_port_names: dest_ports.iter().map(String::as_str).collect(),
                duration: *duration,
                volume: *volume,
            };

            let amplitude = 0.3;
            let sample_rate = 44_100;

            let signal: Box<dyn FiniteSignal<Item = f32>> = match *type_ {
                SignalType::WhiteNoise => Box::new(
                    WhiteNoise::with_amplitude(amplitude).take_duration(sample_rate, *duration),
                ),
                SignalType::PinkNoise => Box::new(
                    PinkNoise::with_amplitude(amplitude).take_duration(sample_rate, *duration),
                ),
                SignalType::LogSweep {
                    start_frequency,
                    end_frequency,
                } => {
                    let sweep = LinearSineSweep::new(
                        start_frequency,
                        end_frequency,
                        config.duration.into(),
                        amplitude,
                        sample_rate,
                    );
                    println!("{}", sweep.len());
                    Box::new(sweep)
                }
            };

            if let Some(file_path) = file_path {
                write_signal_to_file(signal, path::Path::new(file_path))
            } else {
                let jack_client_name = env!("CARGO_BIN_NAME");
                play_signal(jack_client_name, signal, &config)
            }
        }
        Command::Rms { input_port } => {
            let jack_client_name = env!("CARGO_BIN_NAME");
            let (jack_client, _status) =
                jack::Client::new(jack_client_name, jack::ClientOptions::NO_START_SERVER)?;

            meter_rms(jack_client, input_port)
        }
        Command::RunMeasurement {
            duration,
            volume,
            dest_ports,
            input_port,
            type_,
            file_path,
        } => {
            let jack_client_name = env!("CARGO_BIN_NAME");
            let (jack_client, _status) =
                jack::Client::new(jack_client_name, jack::ClientOptions::NO_START_SERVER)?;

            let sample_rate = jack_client.sample_rate() as u32;

            let config = PlaySignalConfig {
                out_port_name: "signal_out",
                dest_port_names: dest_ports.iter().map(String::as_str).collect(),
                duration: *duration,
                volume: *volume,
            };

            let signal: Box<dyn FiniteSignal<Item = f32>> = match *type_ {
                SignalType::WhiteNoise => {
                    Box::new(WhiteNoise::default().take_duration(sample_rate, *duration))
                }
                SignalType::PinkNoise => {
                    Box::new(PinkNoise::default().take_duration(sample_rate, *duration))
                }
                SignalType::LogSweep {
                    start_frequency,
                    end_frequency,
                } => {
                    let sweep = LinearSineSweep::new(
                        start_frequency,
                        end_frequency,
                        config.duration.into(),
                        1.0,
                        sample_rate,
                    );
                    println!("{}", sweep.len());
                    Box::new(sweep)
                }
            };

            run_measurement(
                jack_client,
                &config,
                signal,
                input_port,
                path::Path::new(file_path),
            )
        }
        //        Command::ComputeRIR => {
        //            let mut record_path = String::from(RESULT_PATH);
        //            record_path.push_str("/recorded.wav");
        //            let mut sweep_path = String::from(RESULT_PATH);
        //            sweep_path.push_str("/sweep.wav");
        //
        //            compute_rir(&record_path, &sweep_path)
        //        }
        //        Command::Plot => plot_fake_impulse_respons(),
        //        Command::PingPong => ping_pong(&host, &device, config),
        //        Command::RIR => old_rir(&host, &device, config),
        _ => panic!("Not implemented!"),
    }
}
