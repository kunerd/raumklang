// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/icons.toml
// e06e6ed8ae0dd802393e4523d83a742b4559aef0d8eff1eb1505292f975784e5
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

fn icon(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("icons"))
}
