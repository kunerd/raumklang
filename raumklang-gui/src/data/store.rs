use std::{collections::HashMap, slice};

pub struct Store<L, O> {
    all: Vec<MeasurementState<usize, usize>>,
    loaded: IdMap<L>,
    not_loaded: IdMap<O>,
}

#[derive(Debug)]
struct IdMap<T> {
    next_id: usize,
    data: HashMap<usize, T>,
}

#[derive(Debug)]
pub enum MeasurementState<L, O> {
    Loaded(L),
    NotLoaded(O),
}

impl<L, O> Store<L, O> {
    pub fn new() -> Self {
        Self {
            not_loaded: IdMap::new(),
            loaded: IdMap::new(),
            all: Vec::new(),
        }
    }

    pub fn insert(&mut self, state: MeasurementState<L, O>) {
        let id = match state {
            MeasurementState::NotLoaded(om) => {
                let id = self.not_loaded.insert(om);
                MeasurementState::NotLoaded(id)
            }
            MeasurementState::Loaded(m) => {
                let id = self.loaded.insert(m);
                MeasurementState::Loaded(id)
            }
        };

        self.all.push(id);
    }

    pub fn get_loaded(&self, id: &usize) -> Option<&L> {
        self.loaded.get(id)
    }

    pub fn iter(&self) -> Iter<L, O> {
        Iter {
            inner: self.all.iter(),
            loaded: &self.loaded,
            not_loaded: &self.not_loaded,
        }
    }

    pub fn loaded(&self) -> impl Iterator<Item = &L> {
        self.iter().filter_map(|m| match m {
            MeasurementState::NotLoaded(_) => None,
            MeasurementState::Loaded(m) => Some(m),
        })
    }

    pub fn is_loaded_empty(&self) -> bool {
        self.loaded.is_empty()
    }

    pub fn remove(&mut self, id: usize) -> Option<MeasurementState<L, O>> {
        let state = self.all.remove(id);

        match state {
            MeasurementState::Loaded(id) => self.loaded.remove(&id).map(MeasurementState::Loaded),
            MeasurementState::NotLoaded(id) => {
                self.not_loaded.remove(&id).map(MeasurementState::NotLoaded)
            }
        }
    }
}

pub struct Iter<'a, L, O> {
    inner: slice::Iter<'a, MeasurementState<usize, usize>>,
    loaded: &'a IdMap<L>,
    not_loaded: &'a IdMap<O>,
}

impl<'a, L, O> Iterator for Iter<'a, L, O> {
    type Item = MeasurementState<&'a L, &'a O>;

    fn next(&mut self) -> Option<Self::Item> {
        self.inner.next().map(|state| match state {
            MeasurementState::NotLoaded(id) => {
                MeasurementState::NotLoaded(self.not_loaded.get(id).unwrap())
            }
            MeasurementState::Loaded(id) => MeasurementState::Loaded(self.loaded.get(id).unwrap()),
        })
    }
}

impl<T> IdMap<T> {
    fn new() -> Self {
        Self {
            next_id: 0,
            data: HashMap::new(),
        }
    }

    fn insert(&mut self, entry: T) -> usize {
        let cur_id = self.next_id;

        self.data.insert(cur_id, entry);
        self.next_id += 1;

        cur_id
    }

    fn get(&self, key: &usize) -> Option<&T> {
        self.data.get(key)
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }

    fn remove(&mut self, id: &usize) -> Option<T> {
        self.data.remove(id)
    }
}
