use crate::{
    icon,
    ui::{self, measurement, Loopback},
    widget::sidebar,
};

use chrono::{DateTime, Utc};
use iced::{
    widget::{button, column, right, row, rule, text, tooltip},
    Alignment::Center,
    Element,
    Length::{self, Fill},
};
use raumklang_core::WavLoadError;
use rfd::FileHandle;

use std::{fmt::Display, path::Path, sync::Arc};

#[derive(Debug, Clone)]
pub enum Message {
    Select(Selected),
    Load(Kind),
    Loaded(Result<Arc<LoadedKind>, Arc<WavLoadError>>),
    Remove(measurement::Id),
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Selected {
    Loopback,
    Measurement(measurement::Id),
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
    Loopback(ui::measurement::Loopback),
    Normal(ui::measurement::State),
}

pub fn loopback_entry<'a>(
    selected: Option<Selected>,
    loopback: &'a Loopback,
) -> Element<'a, Message> {
    let is_active = selected.is_some_and(|s| matches!(s, Selected::Loopback));

    let info: Element<_> = match &loopback.state {
        measurement::loopback::State::Loaded(loopback) => {
            let dt: DateTime<Utc> = loopback.as_ref().modified.into();
            column![
                text("Last modified:").size(10),
                text!("{}", dt.format("%x %X")).size(10)
            ]
            .into()
        }
        measurement::loopback::State::NotLoaded(err) => tooltip(
            text("Offline").style(text::danger),
            text!("{err}").style(text::danger),
            tooltip::Position::default(),
        )
        .into(),
    };

    let measurement_btn = button(column![text(&loopback.name).size(16)].push(info).spacing(5))
        .on_press_maybe(
            loopback
                .loaded()
                .map(|_| Message::Select(Selected::Loopback)),
        )
        .style(move |theme, status| {
            let background = theme.extended_palette().background;
            let base = button::subtle(theme, status);

            if is_active {
                base.with_background(background.weak.color)
            } else {
                base
            }
        })
        .width(Fill);

    let delete_btn = button(icon::delete())
        // .on_press(Message::RemoveLoopback)
        .width(30)
        .height(30)
        .style(button::danger);

    let content = row![
        measurement_btn,
        rule::vertical(1.0),
        right(delete_btn).width(Length::Shrink).padding([0, 6])
    ];

    sidebar::item(content, is_active)
}

pub fn list_entry<'a>(
    selected: Option<Selected>,
    measurement: &'a measurement::State,
) -> Element<'a, Message> {
    let id = measurement.id();

    let is_active = selected.is_some_and(|s| s == Selected::Measurement(id));

    let info: Element<_> = match &measurement.loaded() {
        None => text("Error").style(text::danger).into(),
        Some(signal) => {
            let dt: DateTime<Utc> = signal.data.modified.into();
            column![
                text("Last modified:").size(10),
                text!("{}", dt.format("%x %X")).size(10)
            ]
        }
        .into(),
    };

    let measurement_btn = button(
        column![
            text(measurement.name()).wrapping(text::Wrapping::WordOrGlyph),
            info
        ]
        .spacing(5),
    )
    .on_press_maybe(
        measurement
            .loaded()
            .map(|_| Message::Select(Selected::Measurement(id))),
    )
    .style(move |theme, status| {
        let background = theme.extended_palette().background;
        let base = button::subtle(theme, status);

        if is_active {
            base.with_background(background.weak.color)
        } else {
            base
        }
    })
    .width(Fill)
    .clip(true);

    let delete_btn = button(icon::delete().align_x(Center).align_y(Center))
        .on_press_with(move || Message::Remove(id))
        .width(30)
        .height(30)
        .style(button::danger);

    let content = row![
        measurement_btn,
        rule::vertical(1.0),
        right(delete_btn).width(Length::Shrink).padding([0, 6])
    ];

    sidebar::item(content, is_active)
}

pub async fn load_measurement(
    path: impl AsRef<Path>,
    kind: Kind,
) -> Result<Arc<LoadedKind>, Arc<WavLoadError>> {
    match kind {
        Kind::Loopback => Ok(LoadedKind::Loopback(
            ui::measurement::Loopback::from_file(path).await,
        )),
        Kind::Normal => Ok(LoadedKind::Normal(
            ui::measurement::State::from_file(path).await,
        )),
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
