use iced::{widget::text, Element};

pub struct FrequencyResponses {}

#[derive(Debug, Clone)]
pub enum Message {}

impl FrequencyResponses {
    pub fn view(&self) -> Element<'_, Message> {
        text("Not implemented, yet!").into()
    }

    pub(crate) fn new() -> Self {
        Self {}
    }
}
