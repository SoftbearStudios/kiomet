// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::prelude::*;
use std::collections::{BTreeMap, HashMap};
use std::fmt::{Debug, Formatter};
use std::hash::{BuildHasher, Hash};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A map that may require O(k) memory where k is the key space, iterate in a non-deterministic
/// order, or require O(n) time to insert items.
pub trait Map<K: Copy, V>: IntoIterator<Item = (K, V)> {
    type Iter<'a>: Iterator<Item = (K, &'a V)>
    where
        Self: 'a,
        V: 'a;
    type IterMut<'a>: Iterator<Item = (K, &'a mut V)>
    where
        Self: 'a,
        V: 'a;

    fn contains(&self, k: K) -> bool {
        self.get(k).is_some()
    }

    fn get(&self, k: K) -> Option<&V>;

    fn get_mut(&mut self, k: K) -> Option<&mut V>;

    fn insert(&mut self, k: K, v: V) -> Option<V>;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn iter(&self) -> Self::Iter<'_>;

    fn iter_mut(&mut self) -> Self::IterMut<'_>;

    #[allow(clippy::type_complexity)]
    fn keys(&self) -> std::iter::Map<Self::Iter<'_>, fn((K, &'_ V)) -> K> {
        self.iter().map(|(k, _)| k)
    }

    fn len(&self) -> usize;

    fn or_default(&mut self, k: K) -> &mut V
    where
        V: Default;

    fn remove(&mut self, k: K) -> Option<V>;

    fn retain(&mut self, f: impl FnMut(K, &mut V) -> bool);

    #[allow(clippy::type_complexity)]
    fn values(&self) -> std::iter::Map<Self::Iter<'_>, fn((K, &'_ V)) -> &'_ V> {
        self.iter().map(|(_, v)| v)
    }

    #[allow(clippy::type_complexity)]
    fn values_mut(&mut self) -> std::iter::Map<Self::IterMut<'_>, fn((K, &'_ mut V)) -> &'_ mut V> {
        self.iter_mut().map(|(_, v)| v)
    }

    /// Verifies that a [`OrdIter`] implementation is correct in debug mode.
    #[doc(hidden)]
    fn verify_ord_iter(&self)
    where
        Self: OrdIter,
        K: PartialOrd,
    {
        use std::any::type_name;
        debug_assert!(
            self.keys().is_sorted(),
            "{} {}",
            type_name::<K>(),
            type_name::<V>()
        )
    }
}

/// A marker trait for [`Map`]s or [`Set`]s that iterate their keys based on [`Ord`].
/// E.g. NOT [`HashMap`].
pub trait OrdIter {}

/// A marker trait for [`Map`]s or [`Set`]s that require O(log n) time or less to insert items.
/// E.g. NOT [`SortedVecMap`].
pub trait Efficient {}

/// A marker trait for [`Map`]s or [`Set`]s that require O(n) memory. E.g. NOT a dense array.
pub trait Sparse {}

macro_rules! impl_map {
    ($collection:ident, $p:ident $(,$bound:ident)* $(; $s:ident: $b:ident)*) => {
        impl<K: Copy $(+ $bound)*, V $(, $s: $b)*> Map<K, V> for $collection<K, V $(, $s)*> {
            type Iter<'a> = std::iter::Map<std::collections::$p::Iter<'a, K, V>, fn((&K, &'a V)) -> (K, &'a V)> where K: 'a, V: 'a $(, $s: 'a)*;
            type IterMut<'a> = std::iter::Map<std::collections::$p::IterMut<'a, K, V>, fn((&K, &'a mut V)) -> (K, &'a mut V)> where K: 'a, V: 'a $(, $s: 'a)*;

            fn get(&self, k: K) -> Option<&V> {
                self.get(&k)
            }

            fn get_mut(&mut self, k: K) -> Option<&mut V> {
                self.get_mut(&k)
            }

            fn insert(&mut self, k: K, v: V) -> Option<V> {
                self.insert(k, v)
            }

            fn iter(&self) -> Self::Iter<'_> {
                self.iter().map(|(k, v)| (*k, v))
            }

            fn iter_mut(&mut self) -> Self::IterMut<'_> {
                self.iter_mut().map(|(k, v)| (*k, v))
            }

            fn len(&self) -> usize {
                self.len()
            }

            fn or_default(&mut self, k: K) -> &mut V where V: Default {
                self.entry(k).or_insert_with(Default::default)
            }

            fn remove(&mut self, k: K) -> Option<V> {
                self.remove(&k)
            }

            fn retain(&mut self, mut f: impl FnMut(K, &mut V) -> bool) {
                self.retain(|&k, v| f(k, v))
            }
        }

        impl<K, V $(, $s)*> Efficient for $collection<K, V $(, $s)*> {}
        impl<K, V $(, $s)*> Sparse for $collection<K, V $(, $s)*> {}
    }
}

impl_map!(HashMap, hash_map, Eq, Hash; S: BuildHasher);
impl_map!(BTreeMap, btree_map, Ord);
impl<K, V> OrdIter for BTreeMap<K, V> {}

impl<K: Copy + PartialEq, V> Map<K, V> for Option<(K, V)> {
    type Iter<'a> = std::iter::Map<std::option::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (K, &'a V)> where K: 'a, V: 'a;
    type IterMut<'a> = std::iter::Map<std::option::IterMut<'a, (K, V)>, fn(&'a mut (K, V)) -> (K, &'a mut V)> where K: 'a, V: 'a;

    fn get(&self, k: K) -> Option<&V> {
        self.as_ref().and_then(|(key, v)| (key == &k).then_some(v))
    }

    fn get_mut(&mut self, k: K) -> Option<&mut V> {
        self.as_mut().and_then(|(key, v)| (key == &k).then_some(v))
    }

    fn insert(&mut self, k: K, v: V) -> Option<V> {
        self.replace((k, v)).map(|(key, value)| {
            if key != k {
                panic!("Option capacity full")
            }
            value
        })
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter().map(|(k, v)| (*k, v))
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.iter_mut().map(|(k, v)| (*k, v))
    }

    fn len(&self) -> usize {
        self.is_some() as usize
    }

    fn or_default(&mut self, k: K) -> &mut V
    where
        V: Default,
    {
        if let Some((key, value)) = self {
            if key != &k {
                panic!("Option capacity full")
            }
            value
        } else {
            &mut self.insert((k, Default::default())).1
        }
    }

    fn remove(&mut self, k: K) -> Option<V> {
        self.as_ref().is_some_and(|(key, _)| key == &k).then(|| {
            let (_, value) = self.take().unwrap();
            value
        })
    }

    fn retain(&mut self, mut f: impl FnMut(K, &mut V) -> bool) {
        if let Some((k, v)) = self {
            if !f(*k, v) {
                *self = None;
            }
        }
    }
}

impl<K, V> OrdIter for Option<(K, V)> {}
impl<K, V> Efficient for Option<(K, V)> {}
impl<K, V> Sparse for Option<(K, V)> {}

/// A very simple map that operates on a sorted [`Vec`].
#[derive(Clone, Serialize, Deserialize, Encode, Decode)]
pub struct SortedVecMap<K, V>(Vec<(K, V)>);

impl<K: Ord, V> SortedVecMap<K, V> {
    fn search(&self, k: K) -> Result<usize, usize> {
        self.0.binary_search_by(|(p, _)| p.cmp(&k))
    }
}

impl<K: Copy + Debug + Ord, V: Debug> Debug for SortedVecMap<K, V> {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        f.debug_map().entries(self.iter()).finish()
    }
}

impl<K, V> Default for SortedVecMap<K, V> {
    fn default() -> Self {
        Self(Default::default())
    }
}

impl<K: Ord, V> FromIterator<(K, V)> for SortedVecMap<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let vec = <Vec<(K, V)>>::from_iter(iter);
        assert!(vec.array_windows::<2>().all(|[(a, _), (b, _)]| a < b)); // TODO insertion sort
        Self(vec)
    }
}

impl<K, V> IntoIterator for SortedVecMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::vec::IntoIter<Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.0.into_iter()
    }
}

impl<K: Copy + Ord, V> Map<K, V> for SortedVecMap<K, V> {
    type Iter<'a> = std::iter::Map<std::slice::Iter<'a, (K, V)>, fn(&'a (K, V)) -> (K, &'a V)> where K: 'a, V: 'a;
    type IterMut<'a> = std::iter::Map<std::slice::IterMut<'a, (K, V)>, fn(&'a mut (K, V)) -> (K, &'a mut V)> where K: 'a, V: 'a;

    fn get(&self, k: K) -> Option<&V> {
        self.search(k).ok().map(|i| &self.0[i].1)
    }

    fn get_mut(&mut self, k: K) -> Option<&mut V> {
        self.search(k).ok().map(|i| &mut self.0[i].1)
    }

    fn insert(&mut self, k: K, v: V) -> Option<V> {
        match self.search(k) {
            Ok(i) => Some(std::mem::replace(&mut self.0[i].1, v)),
            Err(i) => {
                self.0.insert(i, (k, v));
                None
            }
        }
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.0.iter().map(|(k, v)| (*k, v))
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.0.iter_mut().map(|(k, v)| (*k, v))
    }

    fn len(&self) -> usize {
        self.0.len()
    }

    fn or_default(&mut self, k: K) -> &mut V
    where
        V: Default,
    {
        match self.search(k) {
            Ok(i) => &mut self.0[i].1,
            Err(i) => {
                self.0.insert(i, (k, V::default()));
                &mut self.0[i].1
            }
        }
    }

    fn remove(&mut self, k: K) -> Option<V> {
        self.search(k).ok().map(|i| self.0.remove(i).1)
    }

    fn retain(&mut self, mut f: impl FnMut(K, &mut V) -> bool) {
        self.0.retain_mut(|(k, v)| f(*k, v))
    }
}

impl<K, V> OrdIter for SortedVecMap<K, V> {}
impl<K, V> Sparse for SortedVecMap<K, V> {}

/// TODO explain...
#[derive(Clone, Debug, Default, Serialize, Deserialize, Encode, Decode)]
pub struct Wrapper<K, V>(K, V);

impl<K, V> Deref for Wrapper<K, V> {
    type Target = V;

    fn deref(&self) -> &Self::Target {
        &self.1
    }
}

impl<K, V> DerefMut for Wrapper<K, V> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.1
    }
}

impl<K, V> FromIterator<(K, V)> for Wrapper<K, V> {
    fn from_iter<T: IntoIterator<Item = (K, V)>>(iter: T) -> Self {
        let mut iter = iter.into_iter();
        let (k, v) = iter.next().unwrap();
        debug_assert!(iter.next().is_none());
        Self(k, v)
    }
}

impl<K, V> IntoIterator for Wrapper<K, V> {
    type Item = (K, V);
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        Some((self.0, self.1)).into_iter()
    }
}

impl<K: Copy + PartialEq, V> Map<K, V> for Wrapper<K, V> {
    type Iter<'a> = impl Iterator<Item = (K, &'a V)> where K: 'a, V: 'a;
    type IterMut<'a> = impl Iterator<Item = (K, &'a mut V)> where K: 'a, V: 'a;

    fn get(&self, _: K) -> Option<&V> {
        unimplemented!()
    }

    fn get_mut(&mut self, _: K) -> Option<&mut V> {
        unimplemented!()
    }

    fn insert(&mut self, _: K, _: V) -> Option<V> {
        unreachable!();
    }

    fn iter(&self) -> Self::Iter<'_> {
        Some((self.0, &self.1)).into_iter()
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        Some((self.0, &mut self.1)).into_iter()
    }

    fn len(&self) -> usize {
        1
    }

    fn or_default(&mut self, k: K) -> &mut V
    where
        V: Default,
    {
        *self = Self(k, Default::default());
        &mut self.1
    }

    fn remove(&mut self, _: K) -> Option<V> {
        unimplemented!()
    }

    fn retain(&mut self, _: impl FnMut(K, &mut V) -> bool) {
        unimplemented!()
    }
}

impl<K, V> OrdIter for Wrapper<K, V> {}
impl<K, V> Sparse for Wrapper<K, V> {}

pub struct NonexistentMap<K, V>(!, PhantomData<(K, V)>);

impl<K: Copy, V> Map<K, V> for NonexistentMap<K, V> {
    type Iter<'a> = std::iter::Empty<(K, &'a V)> where K: 'a, V: 'a;
    type IterMut<'a> = std::iter::Empty<(K, &'a mut V)> where K: 'a, V: 'a;

    fn get(&self, _: K) -> Option<&V> {
        self.0
    }

    fn get_mut(&mut self, _: K) -> Option<&mut V> {
        self.0
    }

    fn insert(&mut self, _: K, _: V) -> Option<V> {
        self.0
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.0
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.0
    }

    fn len(&self) -> usize {
        self.0
    }

    fn or_default(&mut self, _: K) -> &mut V
    where
        V: Default,
    {
        self.0
    }

    fn remove(&mut self, _: K) -> Option<V> {
        self.0
    }

    fn retain(&mut self, _: impl FnMut(K, &mut V) -> bool) {
        self.0
    }
}

impl<K, V> IntoIterator for NonexistentMap<K, V> {
    type Item = (K, V);
    type IntoIter = std::iter::Empty<(K, V)>;

    fn into_iter(self) -> Self::IntoIter {
        self.0
    }
}

impl<K, V> OrdIter for NonexistentMap<K, V> {}
impl<K, V> Sparse for NonexistentMap<K, V> {}
impl<K, V> Efficient for NonexistentMap<K, V> {}

/// Like [`Map`] but without values.
pub trait Set<K: Copy> {
    type Iter<'a>: Iterator<Item = K>
    where
        Self: 'a;

    fn contains(&self, k: K) -> bool;

    fn insert(&mut self, k: K) -> bool;

    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    fn iter(&self) -> Self::Iter<'_>;

    fn len(&self) -> usize;

    fn remove(&mut self, k: K) -> bool;
}

impl<K: Copy, T: Map<K, ()>> Set<K> for T {
    type Iter<'a> = std::iter::Map<T::Iter<'a>, fn((K, &())) -> K> where T: 'a;

    fn contains(&self, k: K) -> bool {
        self.contains(k)
    }

    fn insert(&mut self, k: K) -> bool {
        self.insert(k, ()).is_some()
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.iter().map(|(k, _)| k)
    }

    fn len(&self) -> usize {
        self.len()
    }

    fn remove(&mut self, k: K) -> bool {
        self.remove(k).is_some()
    }
}
