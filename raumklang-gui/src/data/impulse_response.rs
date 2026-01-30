use std::{future::Future, path::Path, pin::Pin, sync::Arc};

use iced::{
    futures::{future::Shared, FutureExt},
    task::{sipper, Sipper},
};

#[derive(Debug, Clone, Default)]
pub struct ImpulseResponse(State);

#[derive(Debug, Clone)]

pub enum Event {
    Started,
}

#[derive(Debug, Clone, Default)]
enum State {
    #[default]
    None,
    Computing(Shared<Pin<Box<dyn Future<Output = ImpulseResponse> + Send>>>),
    Computed(Arc<raumklang_core::ImpulseResponse>),
}

impl ImpulseResponse {
    pub fn compute(
        self,
        loopback: &raumklang_core::Loopback,
        measurement: &raumklang_core::Measurement,
    ) -> Option<impl Sipper<Self, Self>> {
        if let State::Computing(_) = self.0 {
            return None;
        }

        if let State::Computed(_) = self.0 {
            return None;
        }

        let loopback = loopback.clone();
        let measurement = measurement.clone();

        let sipper = sipper(async move |mut progress| {
            let computation = async {
                let impulse_response = tokio::task::spawn_blocking(move || {
                    raumklang_core::ImpulseResponse::from_signals(&loopback, &measurement)
                })
                .await
                .unwrap()
                .unwrap();

                ImpulseResponse(State::Computed(Arc::new(impulse_response)))
            }
            .boxed();
            let shared = computation.shared();
            progress
                .send(ImpulseResponse(State::Computing(shared.clone())))
                .await;

            shared.await
        });

        Some(sipper)
    }

    pub fn save(
        self,
        path: Arc<Path>,
        loopback: &raumklang_core::Loopback,
        measurement: &raumklang_core::Measurement,
    ) -> impl Sipper<Option<Arc<Path>>, Self> {
        let fut = self.compute(loopback, measurement).unwrap();

        sipper(async move |mut progress| {
            let ir = fut.run(&progress).await;

            progress.send(ir.clone()).await;
            // tokio::task::spawn_blocking(move || {
            //     let spec = hound::WavSpec {
            //         channels: 1,
            //         sample_rate: ir.sample_rate,
            //         bits_per_sample: 32,
            //         sample_format: hound::SampleFormat::Float,
            //     };

            //     let mut writer = hound::WavWriter::create(path, spec).unwrap();
            //     for s in ir.data {
            //         writer.write_sample(s).unwrap();
            //     }
            //     writer.finalize().unwrap();
            // })
            // .await
            // .unwrap();

            Some(path)
        })
    }

    pub fn inner(&self) -> Option<&raumklang_core::ImpulseResponse> {
        match self.0 {
            State::None => None,
            State::Computing(_) => None,
            State::Computed(ref impulse_response) => Some(impulse_response),
        }
    }

    pub fn progress(&self) -> Progress {
        match self.0 {
            State::None => Progress::None,
            State::Computing(_) => Progress::Computing,
            State::Computed(_) => Progress::Computed,
        }
    }

    pub fn into_inner(self) -> Option<Arc<raumklang_core::ImpulseResponse>> {
        let State::Computed(inner) = self.0 else {
            return None;
        };

        Some(inner)
    }
}

#[derive(Debug, Clone)]
pub enum Progress {
    None,
    Computing,
    Computed,
}
