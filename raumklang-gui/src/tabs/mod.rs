pub mod measurements;
pub mod impulse_response;

use iced::{widget::Container, Element, Length};
use iced_aw::TabLabel;

pub use measurements::Measurements;
pub use impulse_response::ImpulseResponseTab;

pub trait Tab {
    type Message;

    fn title(&self) -> String;

    fn label(&self) -> TabLabel;

    fn view(&self) -> Element<'_, Self::Message> {
        Container::new(self.content())
            .width(Length::Fill)
            .height(Length::Fill)
            .into()
    }

    fn content(&self) -> Element<'_, Self::Message>;
}
