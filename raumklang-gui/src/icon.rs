// Generated automatically by iced_fontello at build time.
// Do not edit manually. Source: ../fonts/icons.toml
// 6e97cb019bb413dc7bddd2cda0e72cc1e1a3771813b80f09e89c763fc7b3b003
use iced::Font;
use iced::widget::{Text, text};

pub const FONT: &[u8] = include_bytes!("../fonts/icons.ttf");

pub fn delete<'a>() -> Text<'a> {
    icon("\u{F1F8}")
}

pub fn record<'a>() -> Text<'a> {
    icon("\u{E0A5}")
}

fn icon(codepoint: &str) -> Text<'_> {
    text(codepoint).font(Font::with_name("icons"))
}
