// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/icons.toml
// ab0a75221ab1331dfa54e7d9f56bf56d76258e259213a7d41471a23956e0b579
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
    icon("\u{27F3}")
}

pub fn settings<'a>() -> Text<'a> {
    icon("\u{26EF}")
}

fn icon(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("icons"))
}
