// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/icons.toml
// a31a04bdecfefc20c79309ff6399870e033e64c8b1bb66096a94c9e027c9d20c
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

pub fn delete<'a>() -> Text<'a> {
    icon("\u{F1F8}")
}

pub fn download<'a>() -> Text<'a> {
    icon("\u{1F4E5}")
}

pub fn record<'a>() -> Text<'a> {
    icon("\u{E0A5}")
}

pub fn reset<'a>() -> Text<'a> {
    icon("\u{27F2}")
}

pub fn settings<'a>() -> Text<'a> {
    icon("\u{26EF}")
}

fn icon(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("icons"))
}
