pub mod landing;
pub mod main;

pub use landing::landing;

use iced::{
    widget::{container, text},
    Element, Length,
};

pub enum Screen {
    Loading,
    Landing,
    Main(main::Tab),
}

pub fn loading<'a, Message: 'a>() -> Element<'a, Message> {
    container(text("Loading ...")).center(Length::Fill).into()
}
