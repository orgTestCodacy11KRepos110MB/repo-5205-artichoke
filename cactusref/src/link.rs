use core::ptr::NonNull;
use hashbrown::{hash_map, HashMap};
use std::hash::{Hash, Hasher};

use crate::ptr::{RcBox, RcBoxPtr};

#[derive(Debug, Clone, Copy, Hash, PartialEq, Eq)]
pub enum Kind {
    Forward,
    Backward,
}

pub struct Links<T: ?Sized> {
    registry: HashMap<Link<T>, usize>,
}

impl<T: ?Sized> Links<T> {
    #[inline]
    pub fn insert(&mut self, other: Link<T>) {
        *self.registry.entry(other).or_insert(0) += 1;
    }

    #[inline]
    pub fn remove(&mut self, other: Link<T>, strong: usize) {
        match self.registry.get(&other).copied().unwrap_or_default() {
            count if count <= strong => self.registry.remove(&other),
            count => self.registry.insert(other, count - strong),
        };
    }

    #[inline]
    pub fn clear(&mut self) {
        self.registry.clear()
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.registry.is_empty()
    }

    #[inline]
    pub fn iter(&self) -> hash_map::Iter<Link<T>, usize> {
        self.registry.iter()
    }
}

impl<T: ?Sized> Clone for Links<T> {
    fn clone(&self) -> Self {
        Self {
            registry: self.registry.clone(),
        }
    }
}

impl<T: ?Sized> Default for Links<T> {
    fn default() -> Self {
        Self {
            registry: HashMap::default(),
        }
    }
}

// Using a a tuple struct is about 10% faster than using named fields.
pub struct Link<T: ?Sized>(NonNull<RcBox<T>>, Kind);

impl<T: ?Sized> Link<T> {
    #[inline]
    pub fn forward(ptr: NonNull<RcBox<T>>) -> Self {
        Self(ptr, Kind::Forward)
    }

    #[inline]
    pub fn backward(ptr: NonNull<RcBox<T>>) -> Self {
        Self(ptr, Kind::Backward)
    }

    #[inline]
    pub fn link_kind(&self) -> Kind {
        self.1
    }

    #[inline]
    pub fn as_forward(&self) -> Self {
        Self::forward(self.0)
    }

    #[inline]
    pub fn as_ptr(&self) -> *const RcBox<T> {
        self.0.as_ptr()
    }

    #[inline]
    pub fn into_raw_non_null(self) -> NonNull<RcBox<T>> {
        self.0
    }
}

impl<T: ?Sized> RcBoxPtr<T> for Link<T> {
    fn inner(&self) -> &RcBox<T> {
        unsafe { self.0.as_ref() }
    }
}

impl<T: ?Sized> Copy for Link<T> {}

impl<T: ?Sized> Clone for Link<T> {
    fn clone(&self) -> Self {
        Self(self.0, self.1)
    }
}

impl<T: ?Sized> PartialEq for Link<T> {
    fn eq(&self, other: &Self) -> bool {
        self.0.as_ptr() == other.0.as_ptr() && self.1 == other.1
    }
}

impl<T: ?Sized> Eq for Link<T> {}

impl<T: ?Sized> Hash for Link<T> {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.0.hash(state);
        self.1.hash(state);
    }
}
