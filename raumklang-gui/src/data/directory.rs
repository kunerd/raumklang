use std::path::Path;
use std::sync::LazyLock;

pub fn data() -> &'static Path {
    PROJECT
        .as_ref()
        .map(directories::ProjectDirs::data_dir)
        .unwrap_or(Path::new("./data"))
}

static PROJECT: LazyLock<Option<directories::ProjectDirs>> =
    LazyLock::new(|| directories::ProjectDirs::from("de", "henku", "raumklang"));
