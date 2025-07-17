// pub mod file;

// // pub use file::File;

// use super::{measurement::Loopback, Measurement};

// use iced::futures::future::join_all;

// use std::path::Path;

// #[derive(Debug)]
// pub struct Project {
//     pub loopback: Option<Loopback>,
//     pub measurements: Vec<Measurement>,
// }

// impl Project {
//     pub async fn load(path: impl AsRef<Path>) -> Result<Self, file::Error> {
//         let path = path.as_ref();
//         let project_file = File::load(path).await?;

//         let loopback = match project_file.loopback {
//             Some(loopback) => Loopback::from_file(loopback.path()).await.ok(),
//             None => None,
//         };

//         let measurements = join_all(
//             project_file
//                 .measurements
//                 .iter()
//                 .map(|p| Measurement::from_file(p.path.clone())),
//         )
//         .await
//         .into_iter()
//         .flatten()
//         .collect();

//         Ok(Self {
//             loopback,
//             measurements,
//         })
//     }
// }

// impl Default for Project {
//     fn default() -> Self {
//         Self {
//             loopback: None,
//             measurements: vec![],
//         }
//     }
// }

use std::{
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub loopback: Option<Loopback>,
    pub measurements: Vec<Measurement>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Loopback(pub Measurement);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Measurement {
    pub path: PathBuf,
}

#[derive(thiserror::Error, Debug, Clone)]
pub enum Error {
    #[error("could not load file: {0}")]
    Io(io::ErrorKind),
    #[error("could not parse file: {0}")]
    Json(String),
}

impl Project {
    pub async fn load(path: impl AsRef<Path>) -> Result<Self, Error> {
        let path = path.as_ref();
        let content = tokio::fs::read(path)
            .await
            .map_err(|err| Error::Io(err.kind()))?;

        let project =
            serde_json::from_slice(&content).map_err(|err| Error::Json(err.to_string()))?;

        Ok(project)
    }
}

impl Loopback {
    pub fn path(&self) -> &PathBuf {
        &self.0.path
    }
}
