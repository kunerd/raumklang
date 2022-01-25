extern crate anyhow;
extern crate clap;
extern crate cpal;

use std::{cmp::Ordering, time::Instant};

use clap::{Parser, Subcommand};
use cpal::traits::{DeviceTrait, HostTrait, StreamTrait};
use plotters::prelude::*;
use rustfft::{num_complex::Complex, FftPlanner};

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
    Sweep,
    ListDevices,
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

    let device = if let Some(device_name) = cli.device {
        host.output_devices()?
            .find(|x| x.name().map(|y| y == device_name).unwrap_or(false))
    } else {
        host.default_output_device()
    };

    let device = device.expect("failed to find output device");
    println!("Output device: {}", device.name()?);

    let config = device.default_output_config().unwrap();
    println!("Default output config: {:#?}", config);

    if cli.plot {
        //plot_frequency_response();
    }

    match &cli.subcommand {
        Command::Sweep => match config.sample_format() {
            cpal::SampleFormat::F32 => run::<f32>(&device, &config.into()),
            cpal::SampleFormat::I16 => run::<i16>(&device, &config.into()),
            cpal::SampleFormat::U16 => run::<u16>(&device, &config.into()),
        },
        Command::ListDevices => {
            println!("Devices:");
            for device in host.output_devices()? {
                if let Ok(device_name) = device.name() {
                    println!("{}", device_name);
                }
            }
            Ok(())
        }
    }
}

fn convert_to_frequency_domain() {
    let sine_sweep = SineSweep::new(50.0, 5000.0, 10.0, 0.5, 44100.0);

    let mut planner = FftPlanner::<f32>::new();
    let fft = planner.plan_fft_forward(438718 / 2);

    let mut buffer: Vec<Complex<f32>> = sine_sweep.map(|m| Complex::from(m)).collect();

    fft.process(&mut buffer);
}

fn plot_frequency_response(buffer: Vec<Complex<f32>>) {
    let root_drawing_area = BitMapBackend::new("sweep.png", (6000, 768)).into_drawing_area();
    root_drawing_area.fill(&WHITE).unwrap();
    let mut chart = ChartBuilder::on(&root_drawing_area)
        .set_label_area_size(LabelAreaPosition::Left, 60)
        .set_label_area_size(LabelAreaPosition::Bottom, 60)
        .build_cartesian_2d(0f32..10f32 * 44100f32, -1.2f32..1.2f32)
        .unwrap();

    //let values: Vec<(_, _)> = buffer.iter().enumerate().collect();
    let values: Vec<(_, _)> = buffer
        .iter()
        .enumerate()
        .map(|(n, y)| (n as f32, y.re))
        .collect();
    //let values: Vec<(f32, f32)> = values[50..5000].iter().map(|(n, y)| (*n as f32, y.re)).collect();

    chart
        .configure_mesh()
        .x_labels(20)
        .y_labels(10)
        .disable_mesh()
        .x_label_formatter(&|v| format!("{:.1}", v))
        .y_label_formatter(&|v| format!("{:.1}", v))
        .draw()
        .unwrap();

    chart.draw_series(LineSeries::new(values, &RED)).unwrap();
    //chart.draw_series(LineSeries::new(
    //    sine_sweep.enumerate().map(|(n, y)| {
    //        //let overall_samples = 10.0 * 44100.0;
    //        //((n as f32 / overall_samples), y)
    //        (n as f32, y)
    //    }),
    //    &RED
    //)).unwrap();
}

struct SineSweep {
    sample_rate: f32,
    sample_index: f32,
    amplitude: f32,
    start_frequency: f32,
    end_frequency: f32,
    phase: f32,
    duration: f32,
}

impl SineSweep {
    pub fn new(
        start_frequency: f32,
        end_frequency: f32,
        duration: f32,
        amplitude: f32,
        sample_rate: f32,
    ) -> Self {
        SineSweep {
            sample_rate,
            sample_index: 0f32,
            amplitude,
            start_frequency,
            end_frequency,
            phase: std::f32::consts::PI,
            duration,
        }
    }
}

impl Iterator for SineSweep {
    type Item = f32;

    fn next(&mut self) -> Option<Self::Item> {
        self.sample_index = self.sample_index + 1.0 / self.sample_rate;
        //let frequency = 440.0;
        let frequency = f32::exp(
            f32::ln(self.start_frequency) * (1.0 - self.sample_index / self.duration)
                + f32::ln(self.end_frequency) * (self.sample_index / self.duration),
        );
        self.phase = (self.phase + 2.0 * std::f32::consts::PI * frequency / self.sample_rate)
            % (2.0 * std::f32::consts::PI);
        match frequency.partial_cmp(&self.end_frequency) {
            Some(ordering) => match ordering {
                Ordering::Less => Some(self.amplitude * f32::sin(self.phase)),
                _ => None,
            },
            _ => None,
        }
    }
}

pub fn run<T>(device: &cpal::Device, config: &cpal::StreamConfig) -> Result<(), anyhow::Error>
where
    T: cpal::Sample,
{
    let sample_rate = config.sample_rate.0 as f32;
    let channels = config.channels as usize;

    let mut sweep_iter = SineSweep::new(20f32, 500f32, 10f32, 0.5f32, sample_rate);

    let err_fn = |err| eprintln!("an error occurred on stream: {}", err);

    let (complete_tx, complete_rx) = std::sync::mpsc::sync_channel(1);
    let stream = device.build_output_stream(
        config,
        move |data: &mut [T], _: &cpal::OutputCallbackInfo| {
            for frame in data.chunks_mut(channels) {
                if let Some(sample) = sweep_iter.next() {
                    let value: T = cpal::Sample::from::<f32>(&sample);
                    for sample in frame.iter_mut() {
                        *sample = value;
                    }
                } else {
                    complete_tx.try_send(()).ok();
                    for sample in frame.iter_mut() {
                        *sample = cpal::Sample::from(&0.0);
                    }
                }
            }
        },
        err_fn,
    )?;
    stream.play()?;

    let start_time = Instant::now();
    // Block until playback completes.
    complete_rx.recv().unwrap();
    let duration = start_time.elapsed();
    println!("Sine sweep duration was: {:?}", duration);

    Ok(())
}
