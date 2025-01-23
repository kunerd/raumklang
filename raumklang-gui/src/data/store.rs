use std::{collections::HashMap, hash::Hash, marker::PhantomData, ops::AddAssign, slice};

pub struct Store<L, O> {
    all: Vec<MeasurementState<Id<L>, Id<O>>>,
    loaded: IdMap<L>,
    not_loaded: IdMap<O>,
}

#[derive(Debug)]
struct IdMap<T> {
    next_id: Id<T>,
    data: HashMap<Id<T>, T>,
}

#[derive(Debug)]
pub struct Id<T> {
    inner: usize,
    phantom: PhantomData<T>,
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

    pub fn get(&self, index: usize) -> Option<MeasurementState<&L, &O>> {
        let id = self.all.get(index);

        id.map(|m| match m {
            MeasurementState::Loaded(id) => self.loaded.get(id).map(MeasurementState::Loaded),
            MeasurementState::NotLoaded(id) => {
                self.not_loaded.get(id).map(MeasurementState::NotLoaded)
            }
        })?
    }

    pub fn get_loaded_id(&self, index: usize) -> Option<Id<L>> {
        self.all.get(index).map(|m| match m {
            MeasurementState::Loaded(id) => Some(*id),
            MeasurementState::NotLoaded(_) => None,
        })?
    }

    pub fn get_loaded_by_id(&self, id: &Id<L>) -> Option<&L> {
        self.loaded.get(id)
    }

    pub fn iter(&self) -> Iter<L, O> {
        Iter {
            inner: self.all.iter(),
            loaded: &self.loaded,
            not_loaded: &self.not_loaded,
        }
    }

    pub fn loaded(&self) -> impl Iterator<Item = (&Id<L>, &L)> {
        self.loaded.iter()
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

impl<'a, L, O> From<MeasurementState<&'a L, &O>> for Option<&'a L> {
    fn from(value: MeasurementState<&'a L, &O>) -> Self {
        match value {
            MeasurementState::Loaded(m) => Some(m),
            MeasurementState::NotLoaded(_) => None,
        }
    }
}

pub struct Iter<'a, L, O> {
    inner: slice::Iter<'a, MeasurementState<Id<L>, Id<O>>>,
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
            next_id: Id::new(),
            data: HashMap::new(),
        }
    }

    fn insert(&mut self, entry: T) -> Id<T> {
        let cur_id = self.next_id;

        self.data.insert(cur_id, entry);
        self.next_id += 1;

        cur_id
    }

    fn get(&self, key: &Id<T>) -> Option<&T> {
        self.data.get(key)
    }

    fn iter(&self) -> impl Iterator<Item = (&Id<T>, &T)> {
        self.data.iter()
    }

    fn remove(&mut self, id: &Id<T>) -> Option<T> {
        self.data.remove(id)
    }

    fn is_empty(&self) -> bool {
        self.data.is_empty()
    }
}

impl<T> Id<T> {
    fn new() -> Self {
        Self {
            inner: 0,
            phantom: PhantomData,
        }
    }
}

impl<T> PartialEq for Id<T> {
    fn eq(&self, other: &Self) -> bool {
        self.inner == other.inner
    }
}

impl<T> Eq for Id<T> {}

impl<T> Hash for Id<T> {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        self.inner.hash(state);
    }
}

impl<T> Clone for Id<T> {
    fn clone(&self) -> Self {
        Self {
            inner: self.inner.clone(),
            phantom: PhantomData,
        }
    }
}

impl<T> Copy for Id<T> {}

impl<T> AddAssign<usize> for Id<T> {
    fn add_assign(&mut self, rhs: usize) {
        self.inner += rhs
    }
}

#[cfg(test)]
mod test {
    #[test]
    fn loaded_is_not_mixed_up_with_not_loaded() {}
}
