use raumklang_core::WavLoadError;
use std::{
    collections::{vec_deque, VecDeque},
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentProjects {
    #[serde(skip)]
    max_values: usize,
    projects_path: VecDeque<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectFile {
    pub loopback: Option<ProjectLoopback>,
    pub measurements: Vec<ProjectMeasurement>,
}

impl ProjectFile {
    pub async fn load(path: impl AsRef<Path>) -> (Self, PathBuf) {
        let path = path.as_ref();
        let content = tokio::fs::read(path).await.unwrap();
        // .map_err(|err| FileError::Io(err.kind()))?;

        let project = serde_json::from_slice(&content).unwrap();
        // .map_err(|err| FileError::Json(err.to_string()))?;

        (project, path.to_path_buf())
    }
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectLoopback(ProjectMeasurement);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectMeasurement {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Loopback(pub Measurement<raumklang_core::Loopback>);

#[derive(Debug, Clone)]
pub struct Measurement<D = raumklang_core::Measurement> {
    pub name: String,
    pub path: PathBuf,
    pub data: D,
}

impl ProjectLoopback {
    pub fn new(inner: ProjectMeasurement) -> Self {
        Self(inner)
    }

    pub fn path(&self) -> &PathBuf {
        &self.0.path
    }
}

impl From<&Loopback> for ProjectLoopback {
    fn from(value: &Loopback) -> Self {
        Self(ProjectMeasurement::from(&value.0))
    }
}

impl<D> From<&Measurement<D>> for ProjectMeasurement {
    fn from(value: &Measurement<D>) -> Self {
        Self {
            path: value.path.to_path_buf(),
        }
    }
}

impl From<Loopback> for Measurement {
    fn from(loopback: Loopback) -> Self {
        Self {
            name: loopback.0.name,
            path: loopback.0.path,
            data: raumklang_core::Measurement::from(loopback.0.data),
        }
    }
}

impl FromFile for Loopback {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized,
    {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let data = raumklang_core::Loopback::from_file(path)?;

        let path = path.to_path_buf();
        Ok(Self(Measurement { name, path, data }))
    }
}

pub trait FromFile {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized;
}

impl FromFile for Measurement {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let data = raumklang_core::Measurement::from_file(path)?;

        Ok(Self {
            name,
            path: path.to_path_buf(),
            data,
        })
    }
}

impl RecentProjects {
    pub fn new(max_values: usize) -> Self {
        Self {
            max_values,
            projects_path: VecDeque::new(),
        }
    }

    pub fn insert(&mut self, path: impl AsRef<Path>) {
        let path = path.as_ref();

        if self.len() == self.max_values {
            self.projects_path.pop_back();
        }

        let entry = self
            .projects_path
            .iter()
            .enumerate()
            .find(|(_, s)| *s == path);

        if let Some((index, _)) = entry {
            self.projects_path.remove(index);
        }

        self.projects_path.push_front(path.to_path_buf());
    }

    pub fn get(&self, index: usize) -> Option<&PathBuf> {
        self.projects_path.get(index)
    }

    pub fn len(&self) -> usize {
        self.projects_path.len()
    }

    pub fn iter(&self) -> vec_deque::Iter<'_, PathBuf> {
        self.projects_path.iter()
    }
}

impl IntoIterator for RecentProjects {
    type Item = PathBuf;
    type IntoIter = vec_deque::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.projects_path.into_iter()
    }
}

impl<'a> IntoIterator for &'a RecentProjects {
    type Item = &'a PathBuf;
    type IntoIter = vec_deque::Iter<'a, PathBuf>;

    fn into_iter(self) -> Self::IntoIter {
        self.iter()
    }
}

#[cfg(test)]
mod test {
    use std::{path::PathBuf, str::FromStr};

    use super::RecentProjects;

    #[test]
    fn insert_adds_to_front() {
        let mut recent = RecentProjects::new(10);

        let first = PathBuf::from_str("First").unwrap();
        let second = PathBuf::from_str("Second").unwrap();

        recent.insert(&first);
        recent.insert(&second);

        let mut iter = recent.into_iter();
        assert_eq!(iter.next(), Some(second));
        assert_eq!(iter.next(), Some(first));
    }

    #[test]
    fn insert_replaces_existing_entry() {
        let mut recent = RecentProjects::new(10);

        let first = PathBuf::from_str("First").unwrap();
        let second = PathBuf::from_str("Second").unwrap();

        recent.insert(&first);
        recent.insert(&second);
        recent.insert(&first);

        let mut iter = recent.into_iter();
        assert_eq!(iter.next(), Some(first));
        assert_eq!(iter.next(), Some(second));
    }

    #[test]
    fn insert_pops_out_last_value() {
        let mut recent = RecentProjects::new(2);

        let first = PathBuf::from_str("First").unwrap();
        let second = PathBuf::from_str("Second").unwrap();
        let third = PathBuf::from_str("Third").unwrap();

        assert_eq!(recent.len(), 0);

        recent.insert(&first);
        recent.insert(&second);
        recent.insert(&third);

        assert_eq!(recent.len(), 2);

        let mut iter = recent.into_iter();
        assert_eq!(iter.next(), Some(third));
        assert_eq!(iter.next(), Some(second));
    }
}
