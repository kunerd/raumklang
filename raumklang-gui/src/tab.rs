pub mod landing;
pub mod measurements;

pub use landing::landing;
pub use measurements::Measurements;

use iced::{
    widget::{container, text},
    Element, Length,
};

pub enum Tab {
    Loading,
    Landing,
    Measurements(Measurements),
}

pub fn loading<'a, Message: 'a>() -> Element<'a, Message> {
    container(text("Loading ...")).center(Length::Fill).into()
}
