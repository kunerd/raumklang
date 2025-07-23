use std::{collections::HashMap, mem};

use iced::Task;

use crate::{
    data::{Samples, Window},
    ui,
};

use super::frequency_response;

#[derive(Debug, Default)]
pub struct Analyses(HashMap<ui::measurement::Id, State>);

#[derive(Debug, Clone)]
pub enum Event {
    ImpulseResponseComputed((ui::measurement::Id, ui::ImpulseResponse)),
    FrequencyResponseComputed((ui::measurement::Id, raumklang_core::FrequencyResponse)),
}

pub enum Action {
    None,
    ImpulseResponseComputed,
}

impl Analyses {
    pub fn apply(&mut self, event: Event) -> Action {
        match event {
            Event::ImpulseResponseComputed((id, ir)) => {
                self.0.insert(id, State::ImpulseResponseComputed(ir));
                Action::ImpulseResponseComputed
            }
            Event::FrequencyResponseComputed((id, fr)) => {
                let Some(state) = self.0.get_mut(&id) else {
                    return Action::None;
                };

                match mem::take(state) {
                    State::ImpulseResponseComputing => {
                        self.0.remove(&id);
                    }
                    State::ImpulseResponseComputed(impulse_response)
                    | State::FrequencyResponseComputing(impulse_response)
                    | State::FrequencyResponseComputed {
                        impulse_response, ..
                    } => {
                        *state = State::FrequencyResponseComputed {
                            impulse_response,
                            frequency_response: frequency_response::Item::from_data(fr),
                        };
                    }
                };

                Action::None
            }
        }
    }

    pub fn compute_impulse_response(
        &mut self,
        id: ui::measurement::Id,
        loopback: &raumklang_core::Loopback,
        measurement: &raumklang_core::Measurement,
    ) -> ImpulseResponseComputationState {
        match self.0.get(&id) {
            Some(State::ImpulseResponseComputing) => ImpulseResponseComputationState::Computing,
            Some(State::ImpulseResponseComputed(_))
            | Some(State::FrequencyResponseComputing(_))
            | Some(State::FrequencyResponseComputed { .. }) => {
                ImpulseResponseComputationState::Computed
            }
            None => {
                let (_impulse_response, computation) =
                    ui::impulse_response::State::new(id, loopback.clone(), measurement.clone());

                self.0.insert(id, State::ImpulseResponseComputing);

                let task = Task::perform(computation.run(), Event::ImpulseResponseComputed);
                ImpulseResponseComputationState::Compute(task)
            }
        }
    }

    pub fn compute_frequency_response(
        &mut self,
        id: ui::measurement::Id,
        window: Window<Samples>,
    ) -> FrequencyResponseCompuationState {
        if let Some(state) = self.0.get_mut(&id) {
            let (new_state, task) = match mem::take(state) {
                State::ImpulseResponseComputing => (
                    State::ImpulseResponseComputing,
                    FrequencyResponseCompuationState::NoImpulseResponse,
                ),
                State::ImpulseResponseComputed(impulse_response) => {
                    let computation = ui::frequency_response::Computation::from_impulse_response(
                        id,
                        impulse_response.clone(),
                        window,
                    );

                    let task = Task::perform(computation.run(), Event::FrequencyResponseComputed);

                    (
                        State::FrequencyResponseComputing(impulse_response),
                        FrequencyResponseCompuationState::Compute(task),
                    )
                }
                State::FrequencyResponseComputing(impulse_response) => (
                    State::FrequencyResponseComputing(impulse_response),
                    FrequencyResponseCompuationState::Computing,
                ),
                State::FrequencyResponseComputed {
                    impulse_response,
                    frequency_response,
                } => (
                    State::FrequencyResponseComputed {
                        impulse_response,
                        frequency_response,
                    },
                    FrequencyResponseCompuationState::Computed,
                ),
            };

            *state = new_state;
            task
        } else {
            FrequencyResponseCompuationState::NoImpulseResponse
        }
    }
}

pub enum ImpulseResponseComputationState {
    Compute(Task<Event>),
    Computing,
    Computed,
}

pub enum FrequencyResponseCompuationState {
    NoImpulseResponse,
    Compute(Task<Event>),
    Computing,
    Computed,
}

#[derive(Debug, Default)]
enum State {
    #[default]
    ImpulseResponseComputing,
    ImpulseResponseComputed(ui::ImpulseResponse),
    FrequencyResponseComputing(ui::ImpulseResponse),
    FrequencyResponseComputed {
        impulse_response: ui::ImpulseResponse,
        frequency_response: frequency_response::Item,
    },
}

enum IrState {
    Computing,
    Computed,
}

enum FrState {
    Computing,
    ImpulseResponseComputed,
    Computed,
}
