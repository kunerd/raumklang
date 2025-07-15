use std::collections::HashMap;

use iced::Task;

use crate::data::{self, impulse_response, measurement};

pub struct Store(HashMap<measurement::Id, State>);

impl Store {}

enum State {
    Init(impulse_response::Computation),
    Computing,
    Computed(data::ImpulseResponse),
}

pub struct ImpulseResponse(State);

impl ImpulseResponse {
    pub fn new(computation: impulse_response::Computation) -> Self {
        Self(State::Init(computation))
    }

    #[must_use]
    pub fn compute<'a, Message>(
        &mut self,
        msg: impl FnOnce(Result<(measurement::Id, data::ImpulseResponse), data::Error>) -> Message
            + Send
            + 'static,
    ) -> Task<Message>
    where
        Message: Send + 'static,
    {
        if let State::Init(computation) = std::mem::replace(&mut self.0, State::Computing) {
            Task::perform(computation.run(), msg)
        } else {
            Task::none()
        }
    }
}
