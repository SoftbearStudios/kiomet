// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{
    actor2::ActorId,
    storage::{Efficient, Map, OrdIter, SortedVecMap},
};
use core_protocol::prelude::*;
use glam::{UVec2, Vec2};
use std::{array, cmp::Ordering, collections::BTreeMap};

// NOTE: This is a reimplementation of crate::entities::Entities and kiomet::ChunkMap
// - no longer necessarily a *square*
// - no longer focused on keeping track of entity indices within a chunk
// - more modular; sector map may store entities or any other sector type

#[derive(Copy, Clone, Debug, Hash, Eq, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct SectorId<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> {
    pub x: u8,
    pub y: u8,
}

impl<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> SectorId<WIDTH, HEIGHT, SCALE> {
    fn new(x: u8, y: u8) -> Self {
        debug_assert!((x as usize) < WIDTH);
        debug_assert!((y as usize) < HEIGHT);
        Self { x, y }
    }

    /// Gets center of sector with id.
    pub fn center(&self) -> Vec2 {
        let mut pos = Vec2::new(self.x as f32, self.y as f32);
        pos *= SCALE as f32;
        pos += Vec2::new(WIDTH as f32, HEIGHT as f32) * SCALE as f32 * -0.5 + SCALE as f32 * 0.5;
        debug_assert_eq!(*self, Self::try_from(pos).unwrap());
        pos
    }

    /// Returns true if the [`SectorId`] intersects a circle.
    fn in_radius(&self, center: Vec2, radius: f32) -> bool {
        // Can't be const because using generic.
        let half = SCALE as f32 * 0.5;

        let abs_diff = (self.center() - center).abs();
        if abs_diff.cmpgt(Vec2::splat(half + radius)).any() {
            false
        } else if abs_diff.cmplt(Vec2::splat(half)).any() {
            true
        } else {
            (abs_diff - half).max(Vec2::ZERO).length_squared() < radius.powi(2)
        }
    }

    /// Returns an iterator over all the [`SectorId`]s in a rectangle defined inclusive corners
    /// `start` and `end`.
    ///
    /// **Panics**
    ///
    /// In debug mode if either component of start > end.
    pub fn iter(start: Self, end: Self) -> impl Iterator<Item = Self> {
        debug_assert!(start.x <= end.x && start.y <= end.y);

        // Range inclusive is slow so add 1.
        (start.x..end.x + 1).flat_map(move |x| (start.y..end.y + 1).map(move |y| Self::new(x, y)))
    }

    /// Returns an iterator over all the [`SectorId`]s in a circle.
    pub fn iter_radius(center: Vec2, radius: f32) -> impl Iterator<Item = Self> {
        let start = Self::saturating_from(center - radius);
        let end = Self::saturating_from(center + radius);
        Self::iter(start, end).filter(move |id| id.in_radius(center, radius))
    }

    /// Returns the [`SectorId`] containing `pos`, with `pos` being clamped to the dimensions of the
    /// data structure.
    fn saturating_from(mut pos: Vec2) -> Self {
        pos *= 1.0 / (SCALE as f32);
        pos += Vec2::new(WIDTH as f32, HEIGHT as f32) / 2.0;
        let pos = pos
            .as_uvec2()
            .min(UVec2::new(WIDTH as u32 - 1, HEIGHT as u32 - 1));
        Self {
            x: pos.x as u8,
            y: pos.y as u8,
        }
    }
}

// Required to make [`world::towers::ChunkMap`] implement [`OrdIter`] and lookup y first.
impl<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> Ord
    for SectorId<WIDTH, HEIGHT, SCALE>
{
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

impl<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> PartialOrd
    for SectorId<WIDTH, HEIGHT, SCALE>
{
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

#[derive(Debug)]
pub struct OutOfBounds;

impl<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> TryFrom<Vec2>
    for SectorId<WIDTH, HEIGHT, SCALE>
{
    type Error = OutOfBounds;

    fn try_from(mut pos: Vec2) -> Result<Self, Self::Error> {
        pos *= 1.0 / SCALE as f32;
        pos += Vec2::new(WIDTH as f32, HEIGHT as f32) * 0.5;
        if pos.cmpge(Vec2::ZERO).all() && pos.cmplt(Vec2::new(WIDTH as f32, HEIGHT as f32)).all() {
            // SAFETY: We've checked that both components of pos are >= 0 and at least < u8::MAX.
            unsafe {
                Ok(Self {
                    x: f32::to_int_unchecked(pos.x),
                    y: f32::to_int_unchecked(pos.y),
                })
            }
        } else {
            Err(OutOfBounds)
        }
    }
}

/// A 2D map of sectors.
#[derive(Debug)]
pub struct SectorMap<T, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> {
    sectors: [[Option<T>; WIDTH]; HEIGHT],
}

impl<T, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16>
    SectorMap<T, WIDTH, HEIGHT, SCALE>
{
    pub fn from_fn(mut f: impl FnMut(SectorId<WIDTH, HEIGHT, SCALE>) -> Option<T>) -> Self {
        Self {
            sectors: array::from_fn(|y| array::from_fn(|x| f(SectorId::new(x as u8, y as u8)))),
        }
    }
}

impl<T, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> Default
    for SectorMap<T, WIDTH, HEIGHT, SCALE>
{
    fn default() -> Self {
        Self::from_fn(|_| None)
    }
}

impl<T, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> IntoIterator
    for SectorMap<T, WIDTH, HEIGHT, SCALE>
{
    type Item = (SectorId<WIDTH, HEIGHT, SCALE>, T);
    type IntoIter = impl Iterator<Item = Self::Item>;

    fn into_iter(self) -> Self::IntoIter {
        self.sectors
            .into_iter()
            .enumerate()
            .flat_map(move |(y, ts)| {
                ts.into_iter().enumerate().filter_map(move |(x, sector)| {
                    sector.map(move |c| (SectorId::new(x as u8, y as u8), c))
                })
            })
    }
}

impl<T, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16>
    Map<SectorId<WIDTH, HEIGHT, SCALE>, T> for SectorMap<T, WIDTH, HEIGHT, SCALE>
{
    type Iter<'a> = impl Iterator<Item = (SectorId<WIDTH, HEIGHT, SCALE>, &'a T)> + Clone where T: 'a;
    type IterMut<'a> = impl Iterator<Item = (SectorId<WIDTH, HEIGHT, SCALE>, &'a mut T)> where T: 'a;

    fn get(&self, id: SectorId<WIDTH, HEIGHT, SCALE>) -> Option<&T> {
        self.sectors
            .get(id.y as usize)? // TODO remove ? (no invalid SectorId).
            .get(id.x as usize)?
            .as_ref()
    }

    fn get_mut(&mut self, id: SectorId<WIDTH, HEIGHT, SCALE>) -> Option<&mut T> {
        self.sectors
            .get_mut(id.y as usize)? // TODO remove ? (no invalid SectorId).
            .get_mut(id.x as usize)?
            .as_mut()
    }

    fn insert(&mut self, id: SectorId<WIDTH, HEIGHT, SCALE>, v: T) -> Option<T> {
        std::mem::replace(&mut self.sectors[id.y as usize][id.x as usize], Some(v))
    }

    fn iter(&self) -> Self::Iter<'_> {
        self.sectors.iter().enumerate().flat_map(move |(y, ts)| {
            ts.iter().enumerate().filter_map(move |(x, sector)| {
                sector
                    .as_ref()
                    .map(move |c| (SectorId::new(x as u8, y as u8), c))
            })
        })
    }

    fn iter_mut(&mut self) -> Self::IterMut<'_> {
        self.sectors
            .iter_mut()
            .enumerate()
            .flat_map(move |(y, ts)| {
                ts.iter_mut().enumerate().filter_map(move |(x, sector)| {
                    sector
                        .as_mut()
                        .map(move |c| (SectorId::new(x as u8, y as u8), c))
                })
            })
    }

    fn len(&self) -> usize {
        self.iter().count() // TODO O(1)
    }

    fn or_default(&mut self, id: SectorId<WIDTH, HEIGHT, SCALE>) -> &mut T
    where
        T: Default,
    {
        self.sectors[id.y as usize][id.x as usize].get_or_insert_default()
    }

    fn remove(&mut self, id: SectorId<WIDTH, HEIGHT, SCALE>) -> Option<T> {
        std::mem::replace(&mut self.sectors[id.y as usize][id.x as usize], None)
    }

    fn retain(&mut self, mut f: impl FnMut(SectorId<WIDTH, HEIGHT, SCALE>, &mut T) -> bool) {
        for (y, ts) in self.sectors.iter_mut().enumerate() {
            for (x, sector) in ts.iter_mut().enumerate() {
                if let Some(s) = sector {
                    let sector_id = SectorId::new(x as u8, y as u8);
                    if !f(sector_id, s) {
                        *sector = None
                    }
                }
            }
        }
    }
}

impl<T, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> OrdIter
    for SectorMap<T, WIDTH, HEIGHT, SCALE>
{
}
impl<T, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> Efficient
    for SectorMap<T, WIDTH, HEIGHT, SCALE>
{
}

/// Implemented by contents of `Entities`.
pub trait EntityTrait: 'static {
    fn position(&self) -> Vec2;

    fn sector_id<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16>(
        &self,
    ) -> Result<SectorId<WIDTH, HEIGHT, SCALE>, OutOfBounds> {
        SectorId::try_from(self.position())
    }
}

/// Index of an entity within `Entities`.
#[derive(
    Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Encode, Decode,
)]
pub struct EntityIndex<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> {
    pub sector_id: SectorId<WIDTH, HEIGHT, SCALE>,
    // TODO bitcode_hint vbr (not gamma).
    pub index: u16,
}

impl<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> EntityIndex<WIDTH, HEIGHT, SCALE> {
    pub fn new(sector_id: SectorId<WIDTH, HEIGHT, SCALE>, index: u16) -> Self {
        Self { sector_id, index }
    }

    pub fn changed_sector<E: EntityTrait>(&self, e: &E) -> bool {
        self.sector_id != e.sector_id().unwrap()
    }

    pub fn sector_id(&self) -> SectorId<WIDTH, HEIGHT, SCALE> {
        self.sector_id
    }

    pub fn index(&self) -> u16 {
        self.index
    }
}

/// A single square sector that stores entities.
#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct Entities<E, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> {
    pub inner: Vec<E>,
}

impl<E, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> Default
    for Entities<E, WIDTH, HEIGHT, SCALE>
{
    fn default() -> Self {
        Self {
            inner: Default::default(),
        }
    }
}

impl<E: EntityTrait, const WIDTH: usize, const HEIGHT: usize, const SCALE: u16>
    Entities<E, WIDTH, HEIGHT, SCALE>
{
    /// Creates an empty sector.
    pub const fn new() -> Self {
        Self { inner: Vec::new() }
    }

    /// Returns the number of contained entities.
    pub fn len(&self) -> usize {
        self.inner.len()
    }

    /// Returns `true` if and only if there are no entities.
    pub fn is_empty(&self) -> bool {
        self.inner.is_empty()
    }

    /// Returns size allocated for entities.
    pub fn capacity(&self) -> usize {
        self.inner.capacity()
    }

    /// May reduce the allocation size of a sector if its entity count dropped sufficiently.
    pub fn shrink(&mut self) {
        if self.inner.capacity() > self.inner.len() * 3 {
            let new_size = (self.inner.len() * 3 / 2).next_power_of_two().max(4);
            if new_size < self.inner.capacity() {
                self.inner.shrink_to(new_size);
            }
        }
    }

    pub fn push(&mut self, entity: E) -> Option<u16> {
        let ret = self.inner.len().try_into().ok()?;
        self.inner.push(entity);
        Some(ret)
    }

    pub fn swap_remove(&mut self, index: u16) -> Option<E> {
        if index as usize >= self.inner.len() {
            None
        } else {
            Some(self.inner.swap_remove(index as usize))
        }
    }

    pub fn get(&self, index: u16) -> Option<&E> {
        self.inner.get(index as usize)
    }

    pub fn get_mut(&mut self, index: u16) -> Option<&mut E> {
        self.inner.get_mut(index as usize)
    }

    pub fn iter(&self) -> impl Iterator<Item = (u16, &E)> {
        self.inner.iter().enumerate().map(|(i, e)| (i as u16, e))
    }

    pub fn iter_mut(&mut self) -> impl Iterator<Item = (u16, &mut E)> {
        self.inner
            .iter_mut()
            .enumerate()
            .map(|(i, e)| (i as u16, e))
    }

    pub fn iter_radius<'a>(
        center: Vec2,
        radius: f32,
        get_entities: impl Fn(SectorId<WIDTH, HEIGHT, SCALE>) -> Option<&'a Self>,
    ) -> impl Iterator<Item = (EntityIndex<WIDTH, HEIGHT, SCALE>, &'a E)> {
        let r2 = radius * radius;
        SectorId::<WIDTH, HEIGHT, SCALE>::iter_radius(center, radius).flat_map(move |sector_id| {
            get_entities(sector_id)
                .into_iter()
                .flat_map(move |entities| {
                    entities
                        .inner
                        .iter()
                        .enumerate()
                        .filter(move |(_, e)| e.position().distance_squared(center) <= r2)
                        .map(move |(index, entity)| {
                            (
                                EntityIndex {
                                    sector_id,
                                    index: index as u16,
                                },
                                entity,
                            )
                        })
                })
        })
    }
}

impl<const WIDTH: usize, const HEIGHT: usize, const SCALE: u16> ActorId
    for SectorId<WIDTH, HEIGHT, SCALE>
{
    type DenseMap<T> = SectorMap<T, WIDTH, HEIGHT, SCALE>;
    type SparseMap<T> = BTreeMap<Self, T>; // TODO SparseChunkMap
    type Map<T> = SortedVecMap<Self, T>;
}
