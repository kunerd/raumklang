use iced::widget::text;
use iced_aw::TabLabel;

use super::Tab;

#[derive(Debug, Default)]
pub struct ImpulseResponse;

#[derive(Debug, Clone)]
pub enum Message {}

impl Tab for ImpulseResponse {
    type Message = Message;

    fn title(&self) -> String {
        "Impulse Response".to_string()
    }

    fn label(&self) -> iced_aw::TabLabel {
        TabLabel::Text(self.title())
    }

    fn content(&self) -> iced::Element<'_, Self::Message> {
        text("Not implemented, yet!").into()
    }
    
}
