// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::prelude::*;
use glam::{UVec2, Vec2};
use maybe_parallel_iterator::{
    IntoMaybeParallelIterator, IntoMaybeParallelRefIterator, IntoMaybeParallelRefMutIterator,
};
use serde::{Deserialize, Serialize};
use std::array;
use std::convert::TryFrom;
use std::fmt::Debug;
use std::ops::{Index, IndexMut};

pub trait EntityTrait {
    fn position(&self) -> Vec2;

    fn sector_id<const SIZE: usize, const SCALE: u16>(
        &self,
    ) -> Result<SectorId<SIZE, SCALE>, OutOfBounds> {
        SectorId::try_from(self.position())
    }
}

/// An efficient collection of entities.
pub struct Entities<E, const SIZE: usize, const SCALE: u16> {
    sectors: [[Option<Sector<E, SIZE, SCALE>>; SIZE]; SIZE],
}

impl<E: EntityTrait + Debug, const SIZE: usize, const SCALE: u16> Debug
    for Entities<E, SIZE, SCALE>
{
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("{")?;
        for sector_id in self.iter_sector_ids() {
            let sector = self.get_sector(sector_id).unwrap();
            write!(f, "{sector_id:?} -> {sector:?},")?;
        }
        f.write_str("}")
    }
}

/// A single square sector, storing the entities within it.
#[derive(Clone, Debug, PartialEq, Hash, Serialize, Deserialize, Encode, Decode)]
pub struct Sector<E, const SIZE: usize, const SCALE: u16> {
    pub entities: Vec<E>,
}

impl<E, const SIZE: usize, const SCALE: u16> Sector<E, SIZE, SCALE> {
    /// Creates an empty sector.
    pub const fn new() -> Self {
        Self {
            entities: Vec::new(),
        }
    }

    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.entities.len()
    }

    pub fn capacity(&self) -> usize {
        self.entities.capacity()
    }

    /// May reduce the allocation size of a sector if its entity count dropped sufficiently.
    fn shrink(&mut self) {
        if self.entities.capacity() > self.entities.len() * 3 {
            let new_size = (self.entities.len() * 3 / 2).next_power_of_two().max(4);
            if new_size < self.entities.capacity() {
                self.entities.shrink_to(new_size);
            }
        }
    }
}

#[derive(
    Copy, Clone, Debug, Hash, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Encode, Decode,
)]
pub struct SectorId<const SIZE: usize, const SCALE: u16>(u8, u8);

impl<const SIZE: usize, const SCALE: u16> SectorId<SIZE, SCALE> {
    /// Gets center of sector with id.
    pub fn center(&self) -> Vec2 {
        let mut pos = Vec2::new(self.0 as f32, self.1 as f32);
        pos *= SCALE as f32;
        pos += SIZE as f32 * SCALE as f32 * -0.5 + SCALE as f32 * 0.5;
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
        debug_assert!(start.0 <= end.0 && start.1 <= end.1);

        // Range inclusive is slow so add 1.
        (start.0..end.0 + 1).flat_map(move |x| (start.1..end.1 + 1).map(move |y| Self(x, y)))
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
        pos += SIZE as f32 / 2.0;
        let pos = pos.as_uvec2().min(UVec2::splat(SIZE as u32 - 1));
        Self(pos.x as u8, pos.y as u8)
    }
}

#[derive(Debug)]
pub struct OutOfBounds;

impl<const SIZE: usize, const SCALE: u16> TryFrom<Vec2> for SectorId<SIZE, SCALE> {
    type Error = OutOfBounds;

    fn try_from(mut pos: Vec2) -> Result<Self, Self::Error> {
        pos *= 1.0 / SCALE as f32;
        pos += SIZE as f32 * 0.5;
        if pos.cmpge(Vec2::ZERO).all() && pos.cmplt(Vec2::splat(SIZE as u8 as f32)).all() {
            // SAFETY: We've checked that both components of pos are >= 0 and at least < u8::MAX.
            unsafe {
                Ok(Self(
                    f32::to_int_unchecked(pos.x),
                    f32::to_int_unchecked(pos.y),
                ))
            }
        } else {
            Err(OutOfBounds)
        }
    }
}

#[derive(
    Copy, Clone, Debug, Ord, PartialOrd, Eq, PartialEq, Serialize, Deserialize, Encode, Decode,
)]
pub struct EntityIndex<const SIZE: usize, const SCALE: u16>(SectorId<SIZE, SCALE>, u16); // TODO bitcode_hint vbr (not gamma).

impl<const SIZE: usize, const SCALE: u16> EntityIndex<SIZE, SCALE> {
    pub fn new(sector_id: SectorId<SIZE, SCALE>, index: usize) -> Self {
        Self(sector_id, index as u16)
    }

    pub fn changed<E: EntityTrait>(&self, e: &E) -> bool {
        self.0 != e.sector_id().unwrap()
    }

    pub fn sector_id(&self) -> SectorId<SIZE, SCALE> {
        self.0
    }

    pub fn index(&self) -> u16 {
        self.1
    }
}

impl<E, const SIZE: usize, const SCALE: u16> Default for Entities<E, SIZE, SCALE> {
    fn default() -> Self {
        Self {
            sectors: array::from_fn(|_| array::from_fn(|_| None::<Sector<E, SIZE, SCALE>>)),
        }
    }
}

impl<E: EntityTrait, const SIZE: usize, const SCALE: u16> Entities<E, SIZE, SCALE> {
    pub fn populate_sectors(&mut self) {
        self.sectors
            .iter_mut()
            .for_each(|s| s.iter_mut().for_each(|s| *s = Some(Sector::new())))
    }

    /// Returns the maximum possible world radius to avoid going out of bounds of this collection.
    pub fn max_world_radius() -> f32 {
        // TODO: Shouldn't need "- 1", but crashes otherwise.
        ((SIZE - 1) / 2) as f32 * SCALE as f32
    }

    pub fn get_sector(&self, sector_id: SectorId<SIZE, SCALE>) -> Option<&Sector<E, SIZE, SCALE>> {
        self.sectors[sector_id.0 as usize][sector_id.1 as usize].as_ref()
    }

    pub fn contains_sector(&self, sector_id: SectorId<SIZE, SCALE>) -> bool {
        self.get_sector(sector_id).is_some()
    }

    pub fn insert_sector(
        &mut self,
        sector_id: SectorId<SIZE, SCALE>,
        sector: Sector<E, SIZE, SCALE>,
    ) -> Option<Sector<E, SIZE, SCALE>> {
        self.sectors[sector_id.0 as usize][sector_id.1 as usize].replace(sector)
    }

    pub fn remove_sector(
        &mut self,
        sector_id: SectorId<SIZE, SCALE>,
    ) -> Option<Sector<E, SIZE, SCALE>> {
        self.sectors[sector_id.0 as usize][sector_id.1 as usize].take()
    }

    pub fn mut_sector(
        &mut self,
        sector_id: SectorId<SIZE, SCALE>,
    ) -> Option<&mut Sector<E, SIZE, SCALE>> {
        self.sectors[sector_id.0 as usize][sector_id.1 as usize].as_mut()
    }

    pub fn add(&mut self, entity: E) -> EntityIndex<SIZE, SCALE> {
        let sector_id = entity.sector_id().unwrap();
        let sector = self.mut_sector(sector_id).expect("TODO");
        let index = EntityIndex(sector_id, sector.entities.len() as u16);
        sector.entities.push(entity);
        index
    }

    pub fn remove(
        &mut self,
        index: EntityIndex<SIZE, SCALE>,
        mut set_index: impl FnMut(&mut E, EntityIndex<SIZE, SCALE>),
    ) -> E {
        let sector_id = index.0;
        let i = index.1 as usize;
        let sector = self.mut_sector(sector_id).expect("TODO");

        let last = sector.entities.len() - 1;
        if i != last {
            set_index(&mut sector.entities[last], index);
        }

        let entity = sector.entities.swap_remove(i);
        sector.shrink();
        entity
    }

    /// Iterates sector ids that are resident in the world.
    pub fn iter_sector_ids(&self) -> impl Iterator<Item = SectorId<SIZE, SCALE>> + '_ {
        self.sectors.iter().enumerate().flat_map(|(x, sectors)| {
            sectors
                .iter()
                .enumerate()
                .filter_map(move |(y, sector)| sector.as_ref().map(|_| SectorId(x as u8, y as u8)))
        })
    }

    /// Iterates all entities.
    pub fn iter(&self) -> impl Iterator<Item = (EntityIndex<SIZE, SCALE>, &E)> {
        self.sectors.iter().enumerate().flat_map(|(x, sectors)| {
            sectors
                .iter()
                .enumerate()
                .filter_map(move |(y, sector)| {
                    sector
                        .as_ref()
                        .map(|sector| (SectorId(x as u8, y as u8), sector))
                })
                .flat_map(move |(sector_id, sector)| {
                    sector
                        .entities
                        .iter()
                        .enumerate()
                        .map(move |(index, entity)| {
                            let entity_index = EntityIndex(sector_id, index as u16);
                            (entity_index, entity)
                        })
                })
        })
    }

    /// Iterates all entities in parallel.
    pub fn par_iter(
        &self,
    ) -> impl IntoMaybeParallelIterator<Item = (EntityIndex<SIZE, SCALE>, &E)> {
        self.sectors
            .maybe_par_iter()
            .enumerate()
            .flat_map(|(x, sectors)| {
                sectors
                    .maybe_par_iter()
                    .enumerate()
                    .filter_map(move |(y, sector)| {
                        sector
                            .as_ref()
                            .map(|sector| (SectorId(x as u8, y as u8), sector))
                    })
                    .flat_map(move |(sector_id, sector)| {
                        sector
                            .entities
                            .maybe_par_iter()
                            .with_min_sequential(256)
                            .enumerate()
                            .map(move |(index, entity)| {
                                let entity_index = EntityIndex(sector_id, index as u16);
                                (entity_index, entity)
                            })
                    })
            })
    }

    /// Mutably iterates all entities in parallel.
    pub fn par_iter_mut(
        &mut self,
    ) -> impl IntoMaybeParallelIterator<Item = (EntityIndex<SIZE, SCALE>, &mut E)> {
        self.sectors
            .maybe_par_iter_mut()
            .enumerate()
            .flat_map(move |(x, sectors)| {
                sectors
                    .maybe_par_iter_mut()
                    .enumerate()
                    .filter_map(move |(y, sector)| {
                        sector
                            .as_mut()
                            .map(|sector| (SectorId(x as u8, y as u8), sector))
                    })
                    .flat_map(|(sector_id, sector)| {
                        sector
                            .entities
                            .maybe_par_iter_mut()
                            .with_min_sequential(256)
                            .enumerate()
                            .map(move |(index, entity)| {
                                let entity_index =
                                    EntityIndex::<SIZE, SCALE>(sector_id, index as u16);
                                (entity_index, entity)
                            })
                    })
            })
    }

    /// Returns an iterator over all the entities in a circle.
    pub fn iter_radius(
        &self,
        center: Vec2,
        radius: f32,
    ) -> impl Iterator<Item = (EntityIndex<SIZE, SCALE>, &E)> {
        let r2 = radius * radius;
        SectorId::iter_radius(center, radius).flat_map(move |sector_id| {
            self.get_sector(sector_id)
                .into_iter()
                .flat_map(move |sector| {
                    sector
                        .entities
                        .iter()
                        .enumerate()
                        .filter(move |(_, e)| e.position().distance_squared(center) <= r2)
                        .map(move |(index, entity)| (EntityIndex(sector_id, index as u16), entity))
                })
        })
    }
}

impl<E: EntityTrait, const SIZE: usize, const SCALE: u16> Index<EntityIndex<SIZE, SCALE>>
    for Entities<E, SIZE, SCALE>
{
    type Output = E;

    fn index(&self, i: EntityIndex<SIZE, SCALE>) -> &Self::Output {
        &self.get_sector(i.0).unwrap().entities[i.1 as usize]
    }
}

impl<E: EntityTrait, const SIZE: usize, const SCALE: u16> IndexMut<EntityIndex<SIZE, SCALE>>
    for Entities<E, SIZE, SCALE>
{
    fn index_mut(&mut self, i: EntityIndex<SIZE, SCALE>) -> &mut Self::Output {
        &mut self.mut_sector(i.0).unwrap().entities[i.1 as usize]
    }
}

#[cfg(test)]
mod tests {
    use crate::entities::{Entities, EntityTrait, SectorId};
    use glam::Vec2;
    use rand::Rng;
    use rand::SeedableRng;
    use rand_chacha::ChaCha20Rng;
    use test::bench::black_box;
    use test::Bencher;

    #[test]
    fn test_sector_id_iter_radius() {
        fn bitmap_to_string(bitmap: [[bool; SIZE]; SIZE]) -> String {
            let mut s = String::new();
            for y in bitmap {
                for x in y {
                    s.push(if x { '1' } else { '0' })
                }
                s.push('\n');
            }
            s
        }

        let entities = test_entities();
        let center = Vec2::ZERO;
        let radius = WORLD_SIZE * 0.25;

        let mut iter_radius = [[false; SIZE]; SIZE];
        for id in TestSectorId::iter_radius(center, radius) {
            iter_radius[id.1 as usize][id.0 as usize] = true;
        }
        let iter_radius = bitmap_to_string(iter_radius);

        let mut in_radius = [[false; SIZE]; SIZE];
        for id in entities.iter_sector_ids() {
            if id.in_radius(center, radius) {
                in_radius[id.1 as usize][id.0 as usize] = true;
            }
        }
        let in_radius = bitmap_to_string(in_radius);

        let expected = "\
0000000000000000
0000000000000000
0000000000000000
0000000000000000
0000011111100000
0000111111110000
0000111111110000
0000111111110000
0000111111110000
0000111111110000
0000111111110000
0000011111100000
0000000000000000
0000000000000000
0000000000000000
0000000000000000\n";

        if iter_radius != expected {
            println!("got:\n{iter_radius}");
            println!("expected:\n{expected}");
            panic!("iter_radius failed");
        }

        if in_radius != in_radius {
            println!("got:\n{in_radius}");
            println!("expected:\n{expected}");
            panic!("in_radius failed");
        }
    }

    #[bench]
    fn bench_sector_id_iter_radius(b: &mut Bencher) {
        let center = Vec2::ZERO;
        let radius = WORLD_SIZE * 0.25;

        b.iter(|| {
            let (center, radius) = black_box((center, radius));
            assert_eq!(
                black_box(TestSectorId::iter_radius(center, radius).count()),
                60
            );
        })
    }

    #[bench]
    fn bench_sector_id_saturating_from(b: &mut Bencher) {
        b.iter(|| {
            for _ in 0..100 {
                black_box(TestSectorId::saturating_from(black_box(Vec2::ONE)));
            }
        });
    }

    #[bench]
    fn bench_sector_id_try_from(b: &mut Bencher) {
        b.iter(|| {
            for _ in 0..100 {
                black_box(TestSectorId::try_from(black_box(Vec2::ONE)).unwrap());
            }
        });
    }

    struct Entity {
        position: Vec2,
        radius: f32,
    }

    impl EntityTrait for Entity {
        fn position(&self) -> Vec2 {
            self.position
        }
    }

    const ENTITY_COUNT: usize = 1000;
    const SIZE: usize = 16;
    const SCALE: u16 = 100;
    const ENTITIES_SIZE: f32 = SIZE as f32 * SCALE as f32;
    const WORLD_SIZE: f32 = 1600.0;
    type TestSectorId = SectorId<SIZE, SCALE>;
    type TestEntities = Entities<Entity, SIZE, SCALE>;

    fn rng() -> impl Rng {
        ChaCha20Rng::from_seed(Default::default())
    }

    fn test_entities() -> TestEntities {
        if ENTITIES_SIZE < WORLD_SIZE {
            panic!("entities too small {} < {}", ENTITIES_SIZE, WORLD_SIZE);
        }

        let mut rng = rng();
        let mut entities = TestEntities::default();
        entities.populate_sectors();
        for _ in 0..ENTITY_COUNT {
            let position = (rng.gen::<Vec2>() - 0.5) * WORLD_SIZE;
            let radius = if rng.gen_bool(0.03) {
                rng.gen::<f32>() * 50.0 + 50.0
            } else {
                rng.gen::<f32>() * 5.0 + 5.0
            };
            entities.add(Entity { position, radius });
        }
        entities
    }

    #[inline(never)]
    fn collide_entities(entities: &TestEntities) -> usize {
        let mut count = 0;
        for (index, entity) in entities.iter() {
            for (other_index, other_entity) in
                entities.iter_radius(entity.position, entity.radius * 2.0)
            {
                // Other entity will handle it.
                if entity.radius < other_entity.radius {
                    continue;
                } else if entity.radius == other_entity.radius {
                    // tiebreaker and ignore duplicates.
                    if index <= other_index {
                        continue;
                    }
                }

                let d2 = entity.position.distance_squared(other_entity.position);
                let r2 = (entity.radius + other_entity.radius).powi(2);
                if d2 < r2 {
                    count += 1;
                }
            }
        }
        count
    }

    #[bench]
    fn bench_entities_collide(b: &mut Bencher) {
        let entities = test_entities();

        b.iter(|| {
            let count = black_box(collide_entities(&entities));
            assert_eq!(count, 420);
        })
    }
}
