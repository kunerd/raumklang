use clap::{Parser, Subcommand};

use raumklang::{
    meter_rms, play_linear_sine_sweep, play_pink_noise, play_white_noise, PlaySignalConfig,
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
        #[command(subcommand)]
        type_: SignalType,
    },
    Plot,
    PingPong,
    Rir,
    RunMeasurement {
        #[clap(short, long)]
        duration: u8,
    },
    ComputeRIR,
    Rms,
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
            type_,
        } => {
            let config = PlaySignalConfig {
                duration: *duration,
                volume: *volume,
            };

            match *type_ {
                SignalType::WhiteNoise => play_white_noise(&config),
                SignalType::PinkNoise => play_pink_noise(&config),
                SignalType::LogSweep {
                    start_frequency,
                    end_frequency,
                } => play_linear_sine_sweep(start_frequency, end_frequency, &config),
            }
        }
        Command::Rms => meter_rms(),
        //        Command::RunMeasurement { duration } => {
        //            let input_device = host.default_input_device().unwrap();
        //            let mut record_path = String::from(RESULT_PATH);
        //            record_path.push_str("/recorded.wav");
        //            let mut sweep_path = String::from(RESULT_PATH);
        //            sweep_path.push_str("/sweep.wav");
        //
        //            run_measurement(&input_device, &record_path, &device, &sweep_path, *duration)
        //        }
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
        //        Command::RMS => {
        //            let input_device = host.default_input_device().unwrap();
        //            meter_rms(&input_device)
        //        }
        _ => panic!("Not implemented!"),
    }
}
