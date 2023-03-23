extern crate anyhow;
extern crate clap;
extern crate cpal;

use clap::{Parser, Subcommand};

use raumklang::{list_devices, list_hosts, play_log_sine_sweep, play_pink_noise, play_white_noise};

#[derive(Parser)]
#[clap(author, version)]
struct Cli {
    #[cfg(all(
        any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
        //feature = "jack"
    ))]
    #[clap(long)]
    jack: bool,
    #[clap(long)]
    device: Option<String>,
    #[clap(long)]
    plot: bool,
    #[command(subcommand)]
    subcommand: Command,
}

#[derive(Subcommand)]
enum Command {
    List(ListCommand),
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

#[derive(Parser)]
struct ListCommand {
    #[clap(subcommand)]
    subcommand: ListSubCommand,
}

#[derive(Subcommand)]
enum ListSubCommand {
    Hosts,
    Devices {
        host: String,
        #[clap(long, short)]
        input: bool,
        #[clap(long, short)]
        output: bool,
    },
}

fn main() -> anyhow::Result<()> {
    let cli = Cli::parse();

    #[cfg(all(any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),))]
    let host = if cli.jack { Some("JACK") } else { None };

    match &cli.subcommand {
        Command::List(command) => run_list_command(command),
        Command::Signal {
            duration,
            volume,
            type_,
        } => {
            let device = cli.device.as_deref();
            match *type_ {
                SignalType::WhiteNoise => play_white_noise(host, device, *duration, *volume),
                SignalType::PinkNoise => play_pink_noise(host, device, *duration, *volume),
                SignalType::LogSweep {
                    start_frequency,
                    end_frequency,
                } => play_log_sine_sweep(
                    host,
                    device,
                    start_frequency,
                    end_frequency,
                    *duration,
                    *volume,
                ),
            }
        }
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

fn run_list_command(command: &ListCommand) -> anyhow::Result<()> {
    match &command.subcommand {
        ListSubCommand::Hosts => list_hosts(),
        ListSubCommand::Devices {
            host,
            input,
            output,
        } => list_devices(host, *input, *output),
    }?;

    Ok(())
}
