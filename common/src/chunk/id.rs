// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chunk::Chunk;
use crate::tower::TowerId;
use kodiak_common::bitcode::{self, *};
use kodiak_common::{U16Vec2, U8Vec2};
use std::cmp::Ordering;
use std::ops::{Add, Deref, DerefMut};

#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq, Encode, Decode)]
pub struct ChunkId(pub U8Vec2);

// Required to make [`world::towers::ChunkMap`] implement [`OrdIter`] and lookup y first.
impl Ord for ChunkId {
    fn cmp(&self, other: &Self) -> Ordering {
        self.y.cmp(&other.y).then_with(|| self.x.cmp(&other.x))
    }

    fn min(self, _: Self) -> Self {
        unimplemented!();
    }

    fn max(self, _: Self) -> Self {
        unimplemented!();
    }
}

impl PartialOrd for ChunkId {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl ChunkId {
    pub fn new(x: u8, y: u8) -> Self {
        Self(U8Vec2::new(x, y))
    }

    #[inline]
    pub fn bottom_left(self) -> TowerId {
        let u16vec2: U16Vec2 = self.0.into();
        TowerId(u16vec2 * Chunk::SIZE as u16)
    }

    pub fn top_right(self) -> TowerId {
        let u16vec2: U16Vec2 = self.0.into();
        let scaled = u16vec2 * Chunk::SIZE as u16;
        TowerId(scaled.add(U16Vec2::splat(Chunk::SIZE as u16 - 1)))
    }
}

impl Deref for ChunkId {
    type Target = U8Vec2;

    fn deref(&self) -> &U8Vec2 {
        &self.0
    }
}

impl DerefMut for ChunkId {
    fn deref_mut(&mut self) -> &mut U8Vec2 {
        &mut self.0
    }
}

impl From<TowerId> for ChunkId {
    #[inline]
    fn from(tower_id: TowerId) -> Self {
        let x = tower_id.x / Chunk::SIZE as u16;
        let y = tower_id.y / Chunk::SIZE as u16;
        Self::new(x as u8, y as u8)
    }
}

/// A [`TowerId`] relative to a [`ChunkId`]. Only 1 byte instead of 4.
#[derive(Copy, Clone, Debug, Default, Hash, Eq, PartialEq, Encode, Decode)]
pub struct RelativeTowerId(pub u8);

impl From<TowerId> for RelativeTowerId {
    #[inline]
    fn from(v: TowerId) -> Self {
        let v = v.0;
        Self(
            (v.x % Chunk::SIZE as u16) as u8 + (v.y % Chunk::SIZE as u16) as u8 * Chunk::SIZE as u8,
        )
    }
}

impl RelativeTowerId {
    #[inline]
    pub(crate) fn to_vec(self) -> U16Vec2 {
        U8Vec2::new(self.0 % Chunk::SIZE as u8, self.0 / Chunk::SIZE as u8).into()
    }

    /// Upgrades a [`RelativeTowerId`] into a [`TowerId`], given a [`ChunkId`].
    #[inline]
    pub fn upgrade(self, chunk_id: ChunkId) -> TowerId {
        let mut tower_id = chunk_id.bottom_left();
        tower_id.0 += self.to_vec();
        tower_id
    }
}

#[cfg(test)]
mod tests {
    use crate::tower::TowerId;

    #[test]
    fn test_relative_tower_id() {
        let tower_id = TowerId::new(123, 456);
        let (chunk_id, relative_id) = tower_id.split();
        assert_eq!(tower_id, relative_id.upgrade(chunk_id))
    }
}
