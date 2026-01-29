use crate::ui::{self};

use raumklang_core::WavLoadError;
use rfd::FileHandle;

use std::{fmt::Display, path::Path, sync::Arc};

#[derive(Debug, Clone)]
pub enum Message {
    Load(Kind),
    Loaded(Result<Arc<LoadedKind>, Arc<WavLoadError>>),
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

#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub enum LoadedKind {
    Loopback(ui::Loopback),
    Normal(ui::Measurement),
}

pub async fn load_measurement(
    path: impl AsRef<Path>,
    kind: Kind,
) -> Result<Arc<LoadedKind>, Arc<WavLoadError>> {
    match kind {
        Kind::Loopback => Ok(LoadedKind::Loopback(
            ui::measurement::Loopback::from_file(path).await,
        )),
        Kind::Normal => Ok(LoadedKind::Normal(ui::Measurement::from_file(path).await)),
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
