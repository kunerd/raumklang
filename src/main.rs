extern crate anyhow;
extern crate clap;
extern crate cpal;


use clap::{Parser, Subcommand};

use raumklang::{
    list_devices, list_hosts, play_sine_sweep, OutputDevice,
};

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
    #[clap(subcommand)]
    subcommand: Command,
}

#[derive(Subcommand)]
enum Command {
    List(ListCommand),
    Sweep {
        #[clap(short, long)]
        duration: u8,
    },
    Plot,
    PingPong,
    RIR,
    RunMeasurement {
        #[clap(short, long)]
        duration: u8,
    },
    ComputeRIR,
    WhiteNoise {
        #[clap(short, long)]
        duration: u8,
    },
    PinkNoise,
    RMS,
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
    // Conditionally compile with jack if the feature is specified.
    #[cfg(all(
        any(target_os = "linux", target_os = "dragonfly", target_os = "freebsd"),
        //feature = "jack"
    ))]
    // Manually check for flags. Can be passed through cargo with -- e.g.
    // cargo run --release --example beep --features jack -- --jack
    let host = if cli.jack {
        cpal::host_from_id(cpal::available_hosts()
            .into_iter()
            .find(|id| *id == cpal::HostId::Jack)
            .expect(
                "make sure --features jack is specified. only works on OSes where jack is available",
            )).expect("jack host unavailable")
    } else {
        cpal::default_host()
    };

    //let device = if let Some(device_name) = cli.device {
    //    host.output_devices()?
    //        .find(|x| x.name().map(|y| y == device_name).unwrap_or(false))
    //} else {
    //    host.default_output_device()
    //};

    //let device = device.expect("failed to find output device");
    ////println!("Output device: {}", device.name()?);

    //let config = device.default_output_config().unwrap();
    ////println!("Default output config: {:#?}", config);

    //const RESULT_PATH: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/results");

    match &cli.subcommand {
        Command::List(command) => run_list_command(command),
        Command::Sweep { duration } => {
            let output = OutputDevice::from_system_default(&host)?;
            play_sine_sweep(&output, *duration)
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
        //        Command::WhiteNoise { duration } => play_white_noise(&device, config, *duration),
        //        Command::PinkNoise => play_pink_noise(&device, config),
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
