use super::Error;

#[derive(Debug, Default, Clone)]
pub enum State {
    #[default]
    Computing,
    Computed(ImpulseResponse),
}

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub max: f32,
    pub sample_rate: u32,
    pub data: Vec<f32>,
}

pub struct Computation {
    id: usize,
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
        }
    }
}

impl Computation {
    pub fn new(
        measurement_id: usize,
        loopback: raumklang_core::Loopback,
        measurement: raumklang_core::Measurement,
    ) -> Self {
        Computation {
            id: measurement_id,
            loopback,
            measurement,
        }
    }

    pub async fn run(self) -> Result<(usize, super::ImpulseResponse), Error> {
        let id = self.id;

        let impulse_response = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&self.loopback, &self.measurement)
                .unwrap()
        })
        .await
        .unwrap();

        Ok((id, impulse_response.into()))
    }
}
