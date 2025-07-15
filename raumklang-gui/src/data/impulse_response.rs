use super::{measurement, Error};

#[derive(Debug, Default, Clone)]
pub enum State {
    #[default]
    Computing,
    Computed(ImpulseResponse),
}
impl State {
    pub(crate) fn new(
        measurement_id: measurement::Id,
        loopback: raumklang_core::Loopback,
        measurement: raumklang_core::Measurement,
    ) -> (Self, Computation) {
        (
            State::Computing,
            Computation::new(measurement_id, loopback, measurement),
        )
    }
}

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub max: f32,
    pub sample_rate: u32,
    pub data: Vec<f32>,
    pub origin: raumklang_core::ImpulseResponse,
}

pub struct Computation {
    measurement_id: measurement::Id,
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
}

impl From<raumklang_core::ImpulseResponse> for ImpulseResponse {
    fn from(impulse_response: raumklang_core::ImpulseResponse) -> Self {
        let data: Vec<_> = impulse_response
            .data
            .iter()
            .map(|s| s.re.powi(2).sqrt())
            .collect();

        let max = data.iter().fold(f32::NEG_INFINITY, |a, b| a.max(*b));

        Self {
            max,
            sample_rate: impulse_response.sample_rate,
            data,
            origin: impulse_response,
        }
    }
}

impl Computation {
    fn new(
        measurement_id: measurement::Id,
        loopback: raumklang_core::Loopback,
        measurement: raumklang_core::Measurement,
    ) -> Self {
        Computation {
            measurement_id,
            loopback,
            measurement,
        }
    }

    pub async fn run(self) -> Result<(measurement::Id, ImpulseResponse), Error> {
        let id = self.measurement_id;

        let impulse_response = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&self.loopback, &self.measurement)
                .unwrap()
        })
        .await
        .unwrap();

        Ok((id, impulse_response.into()))
    }
}
