// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::prelude::*;
use diff::{ArrayDiff, Diff};
use serde::{Deserializer, Serializer};
use serde_big_array::BigArray;
use std::marker::PhantomData;
use std::ops::{Index, IndexMut};
use strum::IntoEnumIterator;

// TODO remove N once generic_const_exprs is complete.
#[derive(Copy, Clone, Debug, Eq, PartialEq, Hash, Encode, Decode)]
pub struct EnumArray<K, V, const N: usize> {
    values: [V; N],
    spooky: PhantomData<K>,
}

impl<K, V: Default, const N: usize> Default for EnumArray<K, V, N> {
    fn default() -> Self {
        Self {
            values: [(); N].map(|_| V::default()),
            spooky: PhantomData,
        }
    }
}

impl<'de, K, V: Serialize + Deserialize<'de>, const N: usize> Serialize for EnumArray<K, V, N> {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        BigArray::serialize(&self.values, serializer)
    }
}

impl<'de, K, V: Serialize + Deserialize<'de>, const N: usize> Deserialize<'de>
    for EnumArray<K, V, N>
{
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        Ok(Self {
            values: BigArray::deserialize(deserializer)?,
            spooky: PhantomData,
        })
    }
}

impl<K, V: Default, const N: usize> EnumArray<K, V, N> {
    pub fn new() -> Self {
        Self::default()
    }
}

impl<K, V, const N: usize> EnumArray<K, V, N>
where
    u8: From<K>,
{
    fn to_idx(k: K) -> usize {
        let i: u8 = k.into();
        i as usize
    }
}

impl<K: IntoEnumIterator, V, const N: usize> EnumArray<K, V, N>
where
    u8: From<K>,
    <K as IntoEnumIterator>::Iterator: DoubleEndedIterator + ExactSizeIterator,
{
    pub fn iter(&self) -> impl Iterator<Item = (K, &V)> + DoubleEndedIterator + '_ {
        K::iter().zip(self.values.iter())
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (K, &mut V)> + DoubleEndedIterator + '_ {
        K::iter().zip(self.values.iter_mut())
    }
}

impl<K: IntoEnumIterator, V, const N: usize> IntoIterator for EnumArray<K, V, N>
where
    u8: From<K>,
{
    type Item = (K, V);
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        K::iter().zip(self.values.into_iter())
    }
}

impl<K: IntoEnumIterator + Copy, V, const N: usize> Index<K> for EnumArray<K, V, N>
where
    u8: From<K>,
{
    type Output = V;
    fn index(&self, index: K) -> &Self::Output {
        &self.values[Self::to_idx(index)]
    }
}

impl<K: IntoEnumIterator + Copy, V, const N: usize> IndexMut<K> for EnumArray<K, V, N>
where
    u8: From<K>,
{
    fn index_mut(&mut self, index: K) -> &mut Self::Output {
        &mut self.values[Self::to_idx(index)]
    }
}

impl<K, V: Default + Diff + PartialEq, const N: usize> diff::Diff for EnumArray<K, V, N> {
    type Repr = ArrayDiff<V>;

    fn diff(&self, other: &Self) -> Self::Repr {
        self.values.diff(&other.values)
    }

    fn apply(&mut self, diff: &Self::Repr) {
        self.values.apply(diff);
    }

    fn identity() -> Self {
        Self::default()
    }
}
