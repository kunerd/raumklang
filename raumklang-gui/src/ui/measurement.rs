pub mod loopback;

pub use loopback::Loopback;

use chrono::{DateTime, Utc};
use iced::{
    Element,
    Length::{Fill, Shrink},
    widget::{button, column, container, right, row, rule, text, tooltip},
};

use std::{
    fmt::Display,
    path::{Path, PathBuf},
    sync::{
        Arc,
        atomic::{self, AtomicUsize},
    },
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
    Loaded(Arc<raumklang_core::Measurement>),
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
            Some(signal) => State::Loaded(Arc::new(signal)),
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

    // FIXME error handling
    pub fn save(&self, path: impl AsRef<Path>) -> impl Future<Output = Option<PathBuf>> {
        let signal = self.signal().cloned();

        let path = path.as_ref().to_path_buf();
        async move {
            tokio::task::spawn_blocking(move || {
                let signal = signal?;

                let spec = hound::WavSpec {
                    channels: 1,
                    sample_rate: signal.sample_rate(),
                    bits_per_sample: 32,
                    sample_format: hound::SampleFormat::Float,
                };

                let mut writer = hound::WavWriter::create(&path, spec).unwrap();
                for s in signal.iter() {
                    writer.write_sample(*s).unwrap();
                }
                writer.finalize().unwrap();

                Some(path)
            })
            .await
            .unwrap()
        }
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

        let file_path = self
            .path
            .as_ref()
            .and_then(|p| p.to_str())
            .unwrap_or_default();

        tooltip(
            sidebar::item(content, active),
            container(text(file_path))
                .padding(5)
                .style(container::bordered_box),
            tooltip::Position::Bottom,
        )
        .into()
    }

    pub fn is_loaded(&self) -> bool {
        match &self.state {
            State::NotLoaded => false,
            State::Loaded { .. } => true,
        }
    }

    pub fn signal(&self) -> Option<&Arc<raumklang_core::Measurement>> {
        match &self.state {
            State::NotLoaded => None,
            State::Loaded(signal) => Some(signal),
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

    pub fn get_mut(&mut self, id: Id) -> Option<&mut Measurement> {
        self.0.iter_mut().find(|m| m.id == id)
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
