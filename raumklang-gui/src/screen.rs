pub mod landing;
pub mod main;

pub use landing::landing;
pub use main::Main;

use iced::{
    widget::{container, text},
    Element, Length,
};

#[allow(clippy::large_enum_variant)]
pub enum Screen {
    Loading,
    Landing,
    Main(Main),
}

pub fn loading<'a, Message: 'a>() -> Element<'a, Message> {
    container(text("Loading ...")).center(Length::Fill).into()
}
