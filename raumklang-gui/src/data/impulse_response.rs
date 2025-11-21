use std::path::{Path, PathBuf};

use iced::task::{sipper, Sipper};

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub origin: raumklang_core::ImpulseResponse,
}

impl ImpulseResponse {
    pub fn from_data(impulse_response: raumklang_core::ImpulseResponse) -> Self {
        Self {
            origin: impulse_response,
        }
    }
}

#[derive(Debug, Clone)]
pub enum Event {
    ComputationStarted,
}

pub fn compute(
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
) -> impl Sipper<ImpulseResponse, Event> {
    sipper(move |mut output| async move {
        output.send(Event::ComputationStarted).await;

        let impulse_response = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement)
        })
        .await
        .unwrap()
        .unwrap();

        ImpulseResponse::from_data(impulse_response)
    })
}

pub fn compute_and_save(
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
) -> impl Sipper<ImpulseResponse, Event> {
    sipper(move |mut output| async move {
        output.send(Event::ComputationStarted).await;

        let impulse_response = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement)
        })
        .await
        .unwrap()
        .unwrap();

        let ir = ImpulseResponse::from_data(impulse_response);
        save_impulse_response(&ir).await;

        ir
    })
}

async fn save_impulse_response(impulse_response: &ImpulseResponse) -> PathBuf {
    let handle = rfd::AsyncFileDialog::new()
        .set_title("Save Impulse Response ...")
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .save_file()
        .await
        .unwrap();

    let spec = hound::WavSpec {
        channels: 1,
        sample_rate: impulse_response.origin.sample_rate.into(),
        bits_per_sample: 32,
        sample_format: hound::SampleFormat::Float,
    };

    let path = handle.path();
    let mut writer = hound::WavWriter::create(path, spec).unwrap();
    for s in impulse_response.origin.data.iter().copied() {
        // FIXME: replace norm
        writer.write_sample(s.norm()).unwrap();
    }
    writer.finalize().unwrap();

    path.to_path_buf()
}
