use std::path::Path;
use std::sync::LazyLock;

pub fn data() -> &'static Path {
    PROJECT
        .as_ref()
        .map(directories::ProjectDirs::data_dir)
        .unwrap_or(Path::new("./data"))
}

pub fn documents() -> &'static Path {
    USER.as_ref()
        .and_then(directories::UserDirs::document_dir)
        .unwrap_or(Path::new("./documents"))
}

static PROJECT: LazyLock<Option<directories::ProjectDirs>> =
    LazyLock::new(|| directories::ProjectDirs::from("de", "henku", "raumklang"));

static USER: LazyLock<Option<directories::UserDirs>> =
    LazyLock::new(|| directories::UserDirs::new());
