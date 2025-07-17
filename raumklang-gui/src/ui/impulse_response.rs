// pub struct Store(HashMap<measurement::Id, State>);

// impl Store {}

// enum State {
//     Init(impulse_response::Computation),
//     Computing,
//     Computed(data::ImpulseResponse),
// }

// pub struct ImpulseResponse(State);

// impl ImpulseResponse {
//     pub fn new(computation: impulse_response::Computation) -> Self {
//         Self(State::Init(computation))
//     }

//     #[must_use]
//     pub fn compute<'a, Message>(
//         &mut self,
//         msg: impl FnOnce(Result<(measurement::Id, data::ImpulseResponse), data::Error>) -> Message
//             + Send
//             + 'static,
//     ) -> Task<Message>
//     where
//         Message: Send + 'static,
//     {
//         if let State::Init(computation) = std::mem::replace(&mut self.0, State::Computing) {
//             Task::perform(computation.run(), msg)
//         } else {
//             Task::none()
//         }
//     }
// }
// use super::{measurement, Error};

use super::measurement;

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

    pub(crate) fn set_computed(&mut self, impulse_response: ImpulseResponse) {
        *self = State::Computed(impulse_response)
    }

    pub fn computed(&self) -> Option<&ImpulseResponse> {
        match self {
            State::Computing => None,
            State::Computed(ref impulse_response) => Some(impulse_response),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ImpulseResponse {
    pub max: f32,
    pub sample_rate: u32,
    pub data: Vec<f32>,
    pub origin: raumklang_core::ImpulseResponse,
}

impl ImpulseResponse {
    fn from_data(impulse_response: raumklang_core::ImpulseResponse) -> Self {
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

pub struct Computation {
    measurement_id: measurement::Id,
    loopback: raumklang_core::Loopback,
    measurement: raumklang_core::Measurement,
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

    pub async fn run(self) -> (measurement::Id, ImpulseResponse) {
        let id = self.measurement_id;

        let impulse_response = tokio::task::spawn_blocking(move || {
            raumklang_core::ImpulseResponse::from_signals(&self.loopback, &self.measurement)
                .unwrap()
        })
        .await
        .unwrap();

        (id, ImpulseResponse::from_data(impulse_response.into()))
    }
}
