use std::{
    collections::{vec_deque, VecDeque},
    path::{Path, PathBuf},
};

use crate::tabs::measurements::WavLoadError;

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentProjects {
    #[serde(skip)]
    max_values: usize,
    projects_path: VecDeque<PathBuf>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct Project {
    pub loopback: Option<ProjectLoopback>,
    pub measurements: Vec<ProjectMeasurement>,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectLoopback(ProjectMeasurement);

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct ProjectMeasurement {
    pub path: PathBuf,
}

#[derive(Debug, Clone)]
pub struct Measurement {
    pub name: String,
    pub path: PathBuf,
    pub sample_rate: u32,
    pub data: Vec<f32>,
}

#[derive(Debug, Clone)]
pub struct Loopback(Measurement);

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

impl From<&Measurement> for ProjectMeasurement {
    fn from(value: &Measurement) -> Self {
        Self {
            path: value.path.to_path_buf(),
        }
    }
}

impl From<Loopback> for Measurement {
    fn from(value: Loopback) -> Self {
        value.0
    }
}

impl Loopback {
    pub fn name(&self) -> &str {
        &self.0.name
    }

    pub fn path(&self) -> &PathBuf {
        &self.0.path
    }

    pub fn sample_rate(&self) -> u32 {
        self.0.sample_rate
    }

    pub fn data(&self) -> &Vec<f32> {
        &self.0.data
    }
}

impl FromFile for Loopback {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized,
    {
        let inner = Measurement::from_file(path)?;
        Ok(Self(inner))
    }
}

pub trait FromFile {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError>
    where
        Self: Sized;
}

impl Measurement {
    pub fn new(name: String, sample_rate: u32, data: Vec<f32>) -> Self {
        Self {
            name,
            path: PathBuf::new(),
            sample_rate,
            data,
        }
    }
}

impl FromFile for Measurement {
    fn from_file(path: impl AsRef<Path>) -> Result<Self, WavLoadError> {
        let path = path.as_ref();
        let name = path
            .file_name()
            .and_then(|n| n.to_os_string().into_string().ok())
            .unwrap_or("Unknown".to_string());

        let mut loopback =
            hound::WavReader::open(path).map_err(|err| map_hound_error(path, err))?;
        let sample_rate = loopback.spec().sample_rate;
        // only mono files
        // currently only 32bit float
        let data = loopback
            .samples::<f32>()
            .collect::<hound::Result<Vec<f32>>>()
            .map_err(|err| map_hound_error(path, err))?;

        Ok(Self {
            name,
            path: path.to_path_buf(),
            sample_rate,
            data,
        })
    }
}

fn map_hound_error(path: impl AsRef<Path>, err: hound::Error) -> WavLoadError {
    let path = path.as_ref().to_path_buf();
    match err {
        hound::Error::IoError(err) => WavLoadError::IoError(path, err.kind()),
        _ => WavLoadError::Other,
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
