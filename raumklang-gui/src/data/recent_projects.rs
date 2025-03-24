use std::{
    collections::{vec_deque, VecDeque},
    io,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RecentProjects {
    #[serde(skip)]
    max_values: usize,
    projects_path: VecDeque<PathBuf>,
}

impl RecentProjects {
    pub fn new(max_values: usize) -> Self {
        Self {
            max_values,
            projects_path: VecDeque::new(),
        }
    }

    async fn path() -> Result<PathBuf, io::Error> {
        Ok(data_dir().await?.join("recent_projects.json"))
    }

    pub async fn load() -> Self {
        let path = Self::path().await.unwrap();

        let content = tokio::fs::read(path).await.unwrap();
        serde_json::from_slice(&content).unwrap()
    }

    pub async fn save(self) {
        let path = Self::path().await.unwrap();

        let contents = serde_json::to_string_pretty(&self).unwrap();
        tokio::fs::write(path, contents).await.unwrap();
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

    pub fn first(&self) -> Option<&PathBuf> {
        self.projects_path.front()
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

async fn data_dir() -> Result<PathBuf, io::Error> {
    let app_dir = directories::ProjectDirs::from("de", "HenKu", "raumklang").unwrap();
    let data_dir = app_dir.data_local_dir().to_path_buf();

    tokio::fs::create_dir_all(&data_dir).await?;

    Ok(data_dir)
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
