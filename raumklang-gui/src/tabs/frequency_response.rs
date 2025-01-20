use iced::{widget::text, Element};

#[derive(Debug, Clone)]
pub enum Message {}

#[derive(Debug, Clone, Default)]
pub struct FrequencyResponse {}

impl FrequencyResponse {
    pub fn view(&self) -> Element<'_, Message> {
        text("Not implemented").into()
    }
}
