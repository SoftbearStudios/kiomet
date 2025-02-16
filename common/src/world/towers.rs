// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chunk::{Chunk, ChunkId};
use crate::tower::{Tower, TowerId, TowerRectangle};
use crate::world::ChunkState;
use kodiak_common::actor_model::*;
use kodiak_common::U16Vec2;
use std::array;

const SIZE: usize = 512;
const SIZE_CHUNKS: usize = SIZE / Chunk::SIZE;

#[derive(Debug)]
pub struct ChunkMap<T> {
    chunks: [[Option<T>; SIZE_CHUNKS]; SIZE_CHUNKS],
}

impl<T> ChunkMap<T> {
    pub fn from_fn(mut f: impl FnMut(ChunkId) -> Option<T>) -> Self {
        Self {
            chunks: array::from_fn(|y| array::from_fn(|x| f(ChunkId::new(x as u8, y as u8)))),
        }
    }
}

impl<T> Default for ChunkMap<T> {
    fn default() -> Self {
        Self::from_fn(|_| None)
    }
}

impl<T> IntoIterator for ChunkMap<T> {
    type Item = (ChunkId, T);
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.chunks
            .into_iter()
            .enumerate()
            .flat_map(move |(y, ts)| {
                ts.into_iter().enumerate().filter_map(move |(x, chunk)| {
                    chunk.map(move |c| (ChunkId::new(x as u8, y as u8), c))
                })
            })
    }
}

impl<T> Map<ChunkId, T> for ChunkMap<T> {
    type Iter<'a> = impl Iterator<Item = (ChunkId, &'a T)> + Clone where T: 'a;
    type IterMut<'a> = impl Iterator<Item = (ChunkId, &'a mut T)> where T: 'a;

    fn get(&self, id: ChunkId) -> Option<&T> {
        self.chunks
            .get(id.y as usize)? // TODO remove ? (no invalid ChunkIds).
            .get(id.x as usize)?
            .as_ref()
    }

    fn get_mut(&mut self, id: ChunkId) -> Option<&mut T> {
        self.chunks
            .get_mut(id.y as usize)? // TODO remove ? (no invalid ChunkIds).
            .get_mut(id.x as usize)?
            .as_mut()
    }

    fn insert(&mut self, id: ChunkId, v: T) -> Option<T> {
        std::mem::replace(&mut self.chunks[id.y as usize][id.x as usize], Some(v))
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.chunks.iter().enumerate().flat_map(move |(y, ts)| {
            ts.iter().enumerate().filter_map(move |(x, chunk)| {
                chunk
                    .as_ref()
                    .map(move |c| (ChunkId::new(x as u8, y as u8), c))
            })
        })
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.chunks.iter_mut().enumerate().flat_map(move |(y, ts)| {
            ts.iter_mut().enumerate().filter_map(move |(x, chunk)| {
                chunk
                    .as_mut()
                    .map(move |c| (ChunkId::new(x as u8, y as u8), c))
            })
        })
    }

    fn len(&self) -> usize {
        self.iter().count() // TODO O(1)
    }

    fn or_default(&mut self, id: ChunkId) -> &mut T
    where
        T: Default,
    {
        self.chunks[id.y as usize][id.x as usize].get_or_insert_default()
    }

    fn remove(&mut self, id: ChunkId) -> Option<T> {
        std::mem::replace(&mut self.chunks[id.y as usize][id.x as usize], None)
    }

    fn retain(&mut self, mut f: impl FnMut(ChunkId, &mut T) -> bool) {
        for (y, ts) in self.chunks.iter_mut().enumerate() {
            for (x, chunk) in ts.iter_mut().enumerate() {
                if let Some(c) = chunk {
                    let chunk_id = ChunkId::new(x as u8, y as u8);
                    if !f(chunk_id, c) {
                        *chunk = None
                    }
                }
            }
        }
    }
}

impl<T> OrdIter for ChunkMap<T> {}
impl<T> Efficient for ChunkMap<T> {}

pub type WorldChunks = ChunkMap<ChunkState>;

impl WorldChunks {
    pub const SIZE: usize = SIZE;
    pub const SIZE_CHUNKS: usize = SIZE_CHUNKS;
    pub const RECTANGLE: TowerRectangle = TowerRectangle::new(
        TowerId(U16Vec2::ZERO),
        TowerId(U16Vec2::splat(Self::SIZE as u16 - 1)),
    );

    pub fn contains(&self, tower_id: TowerId) -> bool {
        self.get(tower_id).is_some()
    }

    pub fn get(&self, tower_id: TowerId) -> Option<&Tower> {
        let (chunk_id, tower_id) = tower_id.split();
        self.get_chunk(chunk_id).and_then(|c| c.get(tower_id))
    }

    pub fn get_chunk(&self, chunk_id: ChunkId) -> Option<&Chunk> {
        Map::get(self, chunk_id).map(|chunk_data| &chunk_data.actor)
    }

    pub fn iter_chunks(&self) -> impl Iterator<Item = (ChunkId, &Chunk)> + Clone {
        self.chunks.iter().enumerate().flat_map(move |(y, ts)| {
            ts.iter().enumerate().filter_map(move |(x, chunk_state)| {
                chunk_state
                    .as_ref()
                    .map(|chunk_state| (ChunkId::new(x as u8, y as u8), &chunk_state.actor))
            })
        })
    }

    pub fn iter_towers(&self) -> impl Iterator<Item = (TowerId, &Tower)> + Clone {
        self.iter_chunks()
            .flat_map(|(chunk_id, chunk)| chunk.iter(chunk_id))
    }

    /// Uses floating point arithmetic.
    pub fn iter_towers_circle(
        &self,
        center: TowerId,
        radius: u16,
    ) -> impl Iterator<Item = (TowerId, &Tower)> + Clone {
        center
            .iter_radius(radius)
            .filter_map(|id| Some(id).zip(self.get(id)))
    }

    pub fn iter_towers_square(
        &self,
        center: TowerId,
        radius: u16,
    ) -> impl Iterator<Item = (TowerId, &Tower)> + Clone {
        self.iter_towers_rectangle(center, U16Vec2::splat(radius * 2))
    }

    /// Iterates towers in a rectangle, while skipping empty or out of bounds towers.
    pub fn iter_towers_rectangle(
        &self,
        center: TowerId,
        dimensions: U16Vec2,
    ) -> impl Iterator<Item = (TowerId, &Tower)> + Clone {
        let half_dim = dimensions / 2;
        let rect = TowerRectangle::new(
            center.0.saturating_sub(half_dim).into(),
            center.0.saturating_add(half_dim).into(),
        );
        self.iter_towers_rect(rect).flatten()
    }

    /// Like iter_towers_rectangle, but it doesn't skip towers.
    pub fn iter_towers_rect(
        &self,
        rect: TowerRectangle,
    ) -> impl Iterator<Item = Option<(TowerId, &Tower)>> + Clone {
        let TowerRectangle {
            bottom_left,
            top_right,
        } = rect;
        (bottom_left.y..=top_right.y).flat_map(move |y| {
            (bottom_left.x..=top_right.x).map(move |x| {
                let tower_id = TowerId::new(x, y);
                Some(tower_id).zip(self.get(tower_id))
            })
        })
    }
}
