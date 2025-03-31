#[derive(Debug)]
pub enum State {
    Computing,
    Computed(FrequencyResponse),
}

#[derive(Debug)]
pub struct FrequencyResponse {}
