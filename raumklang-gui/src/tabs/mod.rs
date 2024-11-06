pub mod signals;
pub mod impulse_resposne;

use iced::{widget::Container, Element, Length};
use iced_aw::TabLabel;

pub use signals::Signals;
pub use impulse_resposne::ImpulseResponse;

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
