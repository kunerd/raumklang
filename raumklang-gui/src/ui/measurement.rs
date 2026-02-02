pub mod loopback;

pub use loopback::Loopback;

use chrono::{DateTime, Utc};
use iced::{
    Element,
    Length::{Fill, Shrink},
    widget::{button, column, right, row, rule, text},
};

use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::atomic::{self, AtomicUsize},
};

use crate::{icon, widget::sidebar};

#[derive(Debug, Clone)]
pub enum Message {
    Select(Selected),
    Remove(Id),
}

#[derive(Debug, Clone, Copy, PartialEq, PartialOrd)]
pub enum Selected {
    Loopback,
    Measurement(Id),
}

#[derive(Debug, Clone)]
pub struct Measurement {
    id: Id,
    pub name: String,
    pub path: Option<PathBuf>,
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Id(usize);

#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
enum State {
    NotLoaded,
    Loaded {
        signal: raumklang_core::Measurement,
        // analysis: Analysis,
    },
}

impl Measurement {
    pub fn new(
        name: String,
        path: Option<PathBuf>,
        signal: Option<raumklang_core::Measurement>,
    ) -> Self {
        static ID: AtomicUsize = AtomicUsize::new(0);
        let id = Id(ID.fetch_add(1, atomic::Ordering::Relaxed));

        let state = match signal {
            Some(signal) => State::Loaded { signal },
            None => State::NotLoaded,
        };

        Self {
            id,
            name,
            path,
            state,
        }
    }

    pub async fn from_file(path: impl AsRef<Path>) -> Self {
        let path = path.as_ref();

        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let signal = raumklang_core::Measurement::from_file(path).ok();

        let path = Some(path.to_path_buf());
        Self::new(name, path, signal)
    }

    pub fn view(&self, active: bool) -> Element<'_, Message> {
        let info: Element<_> = match &self.signal() {
            Some(signal) => {
                let dt: DateTime<Utc> = signal.modified.into();
                column![
                    text("Last modified:").size(10),
                    text!("{}", dt.format("%x %X")).size(10)
                ]
                .into()
            }
            None => text("Offline").style(text::danger).into(),
        };

        let measurement_btn = button(
            column![text(&self.name).wrapping(text::Wrapping::WordOrGlyph), info].spacing(5),
        )
        .on_press_maybe(
            self.is_loaded()
                .then_some(Selected::Measurement(self.id))
                .map(Message::Select),
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
        .width(Fill)
        .clip(true);

        let delete_btn = sidebar::button(icon::delete())
            .style(button::danger)
            .on_press_with(move || Message::Remove(self.id));

        let content = row![
            measurement_btn,
            rule::vertical(1.0),
            right(delete_btn).width(Shrink).padding([0, 6])
        ];

        sidebar::item(content, active)
    }

    pub fn is_loaded(&self) -> bool {
        match &self.state {
            State::NotLoaded => false,
            State::Loaded { .. } => true,
        }
    }

    pub fn signal(&self) -> Option<&raumklang_core::Measurement> {
        match &self.state {
            State::NotLoaded => None,
            State::Loaded { signal, .. } => Some(signal),
        }
    }

    pub(crate) fn id(&self) -> Id {
        self.id
    }
}

#[derive(Debug, Default, Clone)]
pub struct List(Vec<Measurement>);

impl List {
    pub fn iter(&self) -> impl Iterator<Item = &Measurement> + Clone {
        self.0.iter()
    }

    pub fn loaded(&self) -> impl Iterator<Item = &Measurement> {
        self.0.iter().filter(|m| m.is_loaded())
    }

    pub fn push(&mut self, measurement: Measurement) {
        self.0.push(measurement);
    }

    pub fn remove(&mut self, id: Id) -> Option<Measurement> {
        let index = self
            .0
            .iter()
            .enumerate()
            .find(|(_, m)| m.id == id)
            .map(|(i, _)| i)?;

        Some(self.0.remove(index))
    }

    pub fn get(&self, id: Id) -> Option<&Measurement> {
        self.0.iter().find(|m| m.id == id)
    }

    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl Display for Id {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.0)
    }
}
