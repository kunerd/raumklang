use super::{Message, Selected};

use crate::{icon, widget::sidebar};

use iced::{
    widget::{button, column, right, row, rule, text, tooltip},
    Element,
    Length::{Fill, Shrink},
};

use chrono::{DateTime, Utc};

use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

#[derive(Debug, Clone)]
pub struct Loopback {
    pub name: String,
    pub path: Option<PathBuf>,
    state: State,
}

#[derive(Debug, Clone)]
enum State {
    Loaded(raumklang_core::Loopback),
    NotLoaded(Arc<raumklang_core::WavLoadError>),
}

impl Loopback {
    pub(crate) fn is_loaded(&self) -> bool {
        matches!(self.state, State::Loaded(_))
    }

    pub(crate) fn new(name: String, inner: raumklang_core::Loopback) -> Self {
        Self {
            name,
            path: None,
            state: State::Loaded(inner),
        }
    }

    pub async fn from_file(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let state = match raumklang_core::Loopback::from_file(path) {
            Ok(inner) => State::Loaded(inner),
            Err(err) => State::NotLoaded(Arc::new(err)),
        };

        Self {
            name,
            path: Some(path.to_path_buf()),
            state,
        }
    }

    pub fn view(&self, active: bool) -> Element<'_, super::Message> {
        let info: Element<_> = match &self.state {
            State::Loaded(loopback) => {
                let dt: DateTime<Utc> = loopback.as_ref().modified.into();
                column![
                    text("Last modified:").size(10),
                    text!("{}", dt.format("%x %X")).size(10)
                ]
                .into()
            }
            State::NotLoaded(err) => tooltip(
                text("Offline").style(text::danger),
                text!("{err}").style(text::danger),
                tooltip::Position::default(),
            )
            .into(),
        };

        let measurement_btn = button(column![text(&self.name).size(16)].push(info).spacing(5))
            .on_press_maybe(
                self.loaded()
                    .is_some()
                    .then_some(Message::Select(Selected::Loopback)),
            )
            .style(move |theme, status| {
                let background = theme.extended_palette().background;
                let base = button::subtle(theme, status);

                if active {
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
            right(delete_btn).width(Shrink).padding([0, 6])
        ];

        sidebar::item(content, active)
    }

    pub(crate) fn loaded(&self) -> Option<&raumklang_core::Loopback> {
        match &self.state {
            State::Loaded(loopback) => Some(loopback),
            State::NotLoaded(_) => None,
        }
    }
}
