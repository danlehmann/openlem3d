//! Shared things (pictures) kept by id in [`Assets`], held through
//! [`Handle`]s. A thing goes when its last handle does; [`AssetEvent`]s
//! tell when one changed or went, so copies of it (on the GPU) can follow.
//! They are for one reader, which takes them.

use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};

/// What can be kept in [`Assets`].
pub trait Asset: Send + Sync + 'static {}

/// A thing's id: the same for as long as it is kept.
pub struct AssetId<T> {
    index: u64,
    kind: PhantomData<fn() -> T>,
}

impl<T> AssetId<T> {
    const fn new(index: u64) -> Self {
        AssetId {
            index,
            kind: PhantomData,
        }
    }
}

impl<T> Clone for AssetId<T> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<T> Copy for AssetId<T> {}

impl<T> PartialEq for AssetId<T> {
    fn eq(&self, other: &Self) -> bool {
        self.index == other.index
    }
}

impl<T> Eq for AssetId<T> {}

impl<T> Hash for AssetId<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.index.hash(state);
    }
}

impl<T> fmt::Debug for AssetId<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "AssetId({})", self.index)
    }
}

/// Lets the thing go when the last handle to it does.
struct Keep {
    index: u64,
    dropped: Arc<Mutex<Vec<u64>>>,
}

impl Drop for Keep {
    fn drop(&mut self) {
        if let Ok(mut d) = self.dropped.lock() {
            d.push(self.index);
        }
    }
}

/// A hold on a thing in [`Assets`]. [`Handle::default`] holds nothing (and
/// finds nothing).
pub struct Handle<T> {
    id: AssetId<T>,
    keep: Option<Arc<Keep>>,
}

impl<T> Handle<T> {
    pub fn id(&self) -> AssetId<T> {
        self.id
    }
}

impl<T> Default for Handle<T> {
    fn default() -> Self {
        Handle {
            id: AssetId::new(0),
            keep: None,
        }
    }
}

impl<T> Clone for Handle<T> {
    fn clone(&self) -> Self {
        Handle {
            id: self.id,
            keep: self.keep.clone(),
        }
    }
}

impl<T> PartialEq for Handle<T> {
    fn eq(&self, other: &Self) -> bool {
        self.id == other.id
    }
}

impl<T> Eq for Handle<T> {}

impl<T> Hash for Handle<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.id.hash(state);
    }
}

impl<T> fmt::Debug for Handle<T> {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Handle({})", self.id.index)
    }
}

impl<T> From<&Handle<T>> for AssetId<T> {
    fn from(h: &Handle<T>) -> Self {
        h.id
    }
}

impl<T> From<Handle<T>> for AssetId<T> {
    fn from(h: Handle<T>) -> Self {
        h.id
    }
}

/// A thing changing or going.
pub enum AssetEvent<T: Asset> {
    Added { id: AssetId<T> },
    Modified { id: AssetId<T> },
    Removed { id: AssetId<T> },
}

/// Ids start at 1; 0 is [`Handle::default`]'s.
static NEXT: AtomicU64 = AtomicU64::new(1);

pub struct Assets<T: Asset> {
    things: HashMap<AssetId<T>, T>,
    dropped: Arc<Mutex<Vec<u64>>>,
    events: Vec<AssetEvent<T>>,
}

impl<T: Asset> Default for Assets<T> {
    fn default() -> Self {
        Assets {
            things: HashMap::new(),
            dropped: Arc::default(),
            events: Vec::new(),
        }
    }
}

impl<T: Asset> Assets<T> {
    pub fn add(&mut self, thing: impl Into<T>) -> Handle<T> {
        let index = NEXT.fetch_add(1, Ordering::Relaxed);
        let id = AssetId::new(index);
        self.things.insert(id, thing.into());
        self.events.push(AssetEvent::Added { id });
        Handle {
            id,
            keep: Some(Arc::new(Keep {
                index,
                dropped: self.dropped.clone(),
            })),
        }
    }

    pub fn get(&self, id: impl Into<AssetId<T>>) -> Option<&T> {
        self.things.get(&id.into())
    }

    /// The thing, to change it (which tells its copies).
    pub fn get_mut(&mut self, id: impl Into<AssetId<T>>) -> Option<&mut T> {
        let id = id.into();
        let thing = self.things.get_mut(&id)?;
        self.events.push(AssetEvent::Modified { id });
        Some(thing)
    }

    /// Puts `thing` in place of what `id` holds (which tells its copies).
    pub fn insert(&mut self, id: impl Into<AssetId<T>>, thing: T) -> Option<T> {
        let id = id.into();
        let old = self.things.insert(id, thing);
        self.events.push(if old.is_some() {
            AssetEvent::Modified { id }
        } else {
            AssetEvent::Added { id }
        });
        old
    }

    pub fn contains(&self, id: impl Into<AssetId<T>>) -> bool {
        self.things.contains_key(&id.into())
    }

    pub fn len(&self) -> usize {
        self.things.len()
    }

    pub fn is_empty(&self) -> bool {
        self.things.is_empty()
    }

    /// Lets go of the things whose handles are all gone (the runner does
    /// so before each frame).
    pub(crate) fn update(&mut self) {
        let dropped: Vec<u64> = self
            .dropped
            .lock()
            .map(|mut d| std::mem::take(&mut *d))
            .unwrap_or_default();
        for index in dropped {
            let id = AssetId::new(index);
            if self.things.remove(&id).is_some() {
                self.events.push(AssetEvent::Removed { id });
            }
        }
    }

    /// What was added, changed and let go of since the last call.
    pub fn take_events(&mut self) -> Vec<AssetEvent<T>> {
        std::mem::take(&mut self.events)
    }
}
