use crate::{
    data, icon,
    ui::{
        measurement::{self, loopback},
        Loopback,
    },
};

use chrono::{DateTime, Utc};
use iced::{
    widget::{button, column, row, rule, space, text},
    Element, Length,
};
use raumklang_core::WavLoadError;
use rfd::FileHandle;

use std::{fmt::Display, path::Path, sync::Arc};

#[derive(Debug, Clone)]
pub enum Message {
    Select(Selected),
    Load(Kind),
    Loaded(Result<Arc<LoadedKind>, Arc<WavLoadError>>),
    Remove(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum Selected {
    Loopback,
    Measurement(usize),
}

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Loopback,
    Normal,
}

impl Display for Kind {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let text = match self {
            Kind::Loopback => "Loopback",
            Kind::Normal => "Measurement",
        };

        write!(f, "{}", text)
    }
}

#[derive(Debug)]
pub enum LoadedKind {
    Loopback(data::Loopback),
    Normal(data::Measurement),
}

pub fn loopback_entry<'a>(selected: Option<Selected>, signal: &Loopback) -> Element<'a, Message> {
    let info: Element<_> = match &signal.loaded() {
        None => text("Error").style(text::danger).into(),
        Some(data) => {
            let dt: DateTime<Utc> = data.as_ref().modified.into();
            column![
                text("Last modified:").size(10),
                text!("{}", dt.format("%x %X")).size(10)
            ]
        }
        .into(),
    };

    let content = column![
        column![text("Loopback").size(16)].push(info).spacing(5),
        rule::horizontal(2),
        row![
            space::horizontal(),
            button("...").style(button::secondary),
            button(icon::delete())
                // .on_press(Message::RemoveLoopback)
                .style(button::danger)
        ]
        .spacing(3),
    ]
    .clip(true)
    .spacing(5);

    let style = if let Some(Selected::Loopback) = selected {
        button::primary
    } else {
        button::secondary
    };

    button(content)
        .on_press_maybe(signal.loaded().map(|_| Message::Select(Selected::Loopback)))
        .style(style)
        .width(Length::Fill)
        .into()
}

pub fn list_entry<'a>(
    index: usize,
    selected: Option<Selected>,
    signal: &'a measurement::State,
) -> Element<'a, Message> {
    let info: Element<_> = match &signal.loaded() {
        None => text("Error").style(text::danger).into(),
        Some(loaded) => {
            let dt: DateTime<Utc> = loaded.data.modified.into();
            column![
                text("Last modified:").size(10),
                text!("{}", dt.format("%x %X")).size(10)
            ]
        }
        .into(),
    };

    let content = column![
        column![text(signal.name()).size(16),].push(info).spacing(5),
        rule::horizontal(2),
        row![
            space::horizontal(),
            button("...").style(button::secondary),
            button(icon::delete())
                .on_press(Message::Remove(index))
                .style(button::danger)
        ]
        .spacing(3),
    ]
    .clip(true)
    .spacing(5);

    let style = match selected {
        Some(Selected::Measurement(selected)) if selected == index => button::primary,
        _ => button::secondary,
    };

    button(content)
        .on_press_maybe(
            signal
                .loaded()
                .map(|_| Message::Select(Selected::Measurement(index))),
        )
        .width(Length::Fill)
        .style(style)
        .into()
}

pub async fn load_measurement(
    path: impl AsRef<Path>,
    kind: Kind,
) -> Result<Arc<LoadedKind>, Arc<WavLoadError>> {
    match kind {
        Kind::Loopback => data::Loopback::from_file(path)
            .await
            .map(LoadedKind::Loopback),
        Kind::Normal => data::Measurement::from_file(path)
            .await
            .map(LoadedKind::Normal),
    }
    .map(Arc::new)
    .map_err(Arc::new)
}

pub async fn pick_file_and_load_signal(
    file_type: impl AsRef<str>,
    kind: Kind,
) -> Result<Arc<LoadedKind>, Arc<WavLoadError>> {
    let handle = pick_file(file_type).await.unwrap();

    load_measurement(handle.path(), kind).await
}

pub async fn pick_file(file_type: impl AsRef<str>) -> Result<FileHandle, Error> {
    rfd::AsyncFileDialog::new()
        .set_title(format!("Choose {} file", file_type.as_ref()))
        .add_filter("wav", &["wav", "wave"])
        .add_filter("all", &["*"])
        .pick_file()
        .await
        .ok_or(Error::DialogClosed)
}

#[derive(Debug, Clone, thiserror::Error)]
pub enum Error {
    // #[error("error while loading file: {0}")]
    // File(PathBuf, Arc<WavLoadError>),
    #[error("dialog closed")]
    DialogClosed,
}
