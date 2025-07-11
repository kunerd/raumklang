use crate::data::{frequency_response, impulse_response, ImpulseResponse};

#[derive(Debug, Default)]
pub struct Analyses(Vec<Analysis>);
impl Analyses {
    pub(crate) fn clear(&mut self) {
        self.0.clear();
    }

    pub(crate) fn reset_frequency_responses(&mut self) {
        for analysis in &mut self.0 {
            let state = match analysis.state.take() {
                State::None => State::None,
                State::ImpulseResponse(state) => State::ImpulseResponse(state),
                State::FrequencyResponse(impulse_response, _state) => {
                    State::ImpulseResponse(impulse_response::State::Computed(impulse_response))
                }
            };

            analysis.state = state;
        }
    }

    pub(crate) fn impulse_responses(&self) -> Vec<&ImpulseResponse> {
        self.0
            .iter()
            .filter_map(|a| match &a.state {
                State::None => None,
                State::ImpulseResponse(state) => None,
                State::FrequencyResponse(impulse_response, state) => Some(impulse_response),
            })
            .collect()
    }

    // fn impulse_responses_mut(&mut self) -> Vec<&mut ImpulseResponse> {
    //     self.0
    //         .iter_mut()
    //         .filter_map(|a| match &mut a.state {
    //             State::None => None,
    //             State::ImpulseResponse(state) => None,
    //             State::FrequencyResponse(impulse_response, state) => Some(impulse_response),
    //         })
    //         .collect()
    // }
}

#[derive(Debug)]
pub struct Analysis {
    measurement_id: usize,
    state: State,
}

#[derive(Debug, Default)]
pub enum State {
    #[default]
    None,
    ImpulseResponse(impulse_response::State),
    FrequencyResponse(ImpulseResponse, frequency_response::State),
}
impl State {
    fn take(&mut self) -> State {
        std::mem::take(self)
    }
}
