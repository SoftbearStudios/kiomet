// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chunk::{ChunkId, RelativeTowerId};
use crate::tower::{integer_sqrt, TowerRectangle, TowerType};
use crate::world::{World, WorldChunks};
use kodiak_common::bitcode::{self, *};
use kodiak_common::glam::Vec2;
use kodiak_common::{I16Vec2, U16Vec2, U8Vec2};
use std::ops::{Deref, DerefMut};
use std::sync::LazyLock;

// Use 32 bit fnv hash because it's fast.
const FNV_OFFSET: u32 = 2166136261;
const FNV_PRIME: u32 = 16777619;

macro_rules! condense {
    ($input:expr, $t:ty) => {{
        let (low, high) = ($input as $t, ($input >> <$t>::BITS) as $t);
        low ^ high
    }};
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, PartialOrd, Hash, Encode, Decode)]
#[repr(transparent)]
pub struct TowerId(pub U16Vec2);

impl TowerId {
    pub const CONVERSION: u16 = 5;

    #[inline]
    pub const fn new(x: u16, y: u16) -> Self {
        Self(U16Vec2::new(x, y))
    }

    pub fn tower_type(self) -> TowerType {
        let mut hash = FNV_OFFSET;
        let mut write_u8 = |u: u8| {
            hash = hash.wrapping_mul(FNV_PRIME);
            hash ^= u as u32;
        };
        let mut write_u16 = |u: u16| {
            let bytes = u.to_le_bytes();
            write_u8(bytes[0]);
            write_u8(bytes[1]);
        };
        let c = self.0.wrapping_add(U16Vec2::splat(31415)); // Add an amount to be different from OffsetTable.
        write_u16(c.x);
        write_u16(c.y);
        let hash = condense!(condense!(hash, u16), u8);
        TowerType::generate(hash)
    }

    #[inline]
    pub fn split(self) -> (ChunkId, RelativeTowerId) {
        (ChunkId::from(self), RelativeTowerId::from(self))
    }

    pub fn is_valid(self) -> bool {
        self.x <= WorldChunks::SIZE as u16 && self.y <= WorldChunks::SIZE as u16
    }

    pub fn offset(self) -> U16Vec2 {
        OFFSET_TABLE.offset(self)
    }

    pub fn integer_position(self) -> U16Vec2 {
        self.0 * Self::CONVERSION + self.offset()
    }

    /// Returns offset world position in tower cell (id * conv + offset) as [`Vec2`].
    #[inline]
    pub fn as_vec2(self) -> Vec2 {
        self.integer_position().as_vec2()
    }

    /// Returns center world position of tower cell (id * conv + conv / 2) as [`Vec2`].
    #[inline]
    pub fn center_position(self) -> Vec2 {
        (self.0 * Self::CONVERSION).as_vec2() + Self::CONVERSION as f32 * 0.5
    }

    /// Returns min world position of tower cell (id * conv) as [`Vec2`].
    #[inline]
    pub fn floor_position(self) -> Vec2 {
        (self.0 * Self::CONVERSION).as_vec2()
    }

    /// Returns max world position of tower cell ((id + 1) * conv) as [`Vec2`].
    #[inline]
    pub fn ceil_position(self) -> Vec2 {
        ((self.0 + U16Vec2::splat(1)) * Self::CONVERSION).as_vec2()
    }

    /// Opposite of [`Self::center_position`].
    pub fn rounded(world_position: Vec2) -> Self {
        Self(U16Vec2::rounded(
            world_position * (1.0 / (Self::CONVERSION as f32)),
        ))
    }

    /// Opposite of [`Self::floor_position`].
    pub fn floor(world_position: Vec2) -> Self {
        Self(U16Vec2::floor(
            world_position * (1.0 / (Self::CONVERSION as f32)),
        ))
    }

    /// Opposite of [`Self::ceil_position`].
    pub fn ceil(world_position: Vec2) -> Self {
        Self(U16Vec2::ceil(
            world_position * (1.0 / (Self::CONVERSION as f32)),
        ))
    }

    pub fn closest(world_position: Vec2) -> Option<Self> {
        let rounded = Self::rounded(world_position).0;
        let mut closest = None;
        for x in rounded.x.saturating_sub(1)..=rounded.x.saturating_add(1) {
            for y in rounded.y.saturating_sub(1)..=rounded.y.saturating_add(1) {
                let tower_id = TowerId::new(x, y);
                let distance = tower_id.as_vec2().distance(world_position);
                if closest
                    .map(|(_closest_tower_id, closest_distance)| distance < closest_distance)
                    .unwrap_or(true)
                {
                    closest = Some((tower_id, distance));
                }
            }
        }
        closest.map(|(tower_id, _)| tower_id)
    }

    /// Returns true if `other` is a neighbor of `self` (connected by a road).
    pub fn is_neighbor(self, other: Self) -> bool {
        self.neighbor_to(other).is_some()
    }

    /// Returns an iterator over the neighbor towers (i.e. connected by roads).
    pub fn neighbors(self) -> impl Iterator<Item = TowerId> + 'static {
        self.neighbors_enumerated().map(|(_, id)| id)
    }

    /// Same as [`Self::neighbors`], but provides [`TowerNeighbor`] as well.
    pub fn neighbors_enumerated(self) -> impl Iterator<Item = (TowerNeighbor, TowerId)> + 'static {
        let mut bits = NEIGHBOR_TABLE.neighbors(self);
        std::iter::from_fn(move || {
            (bits != 0).then(|| {
                let i = bits.trailing_zeros();
                bits ^= 1 << i;
                let neighbor = TowerNeighbor::try_from(i as u8).unwrap();
                (neighbor, self.neighbor_unchecked(neighbor))
            })
        })
    }

    /// Deterministic distance squared.
    #[inline]
    pub fn distance_squared(self, other: Self) -> u64 {
        let self_world = self.integer_position();
        let other_world = other.integer_position();
        let x_diff = self_world.x.abs_diff(other_world.x) as u32;
        let y_diff = self_world.y.abs_diff(other_world.y) as u32;
        // Squaring doesn't overflow since [`u16::MAX`]^2 = [`u32::MAX`].
        // Adding doesn't overflow [`u64`].
        (x_diff * x_diff) as u64 + (y_diff * y_diff) as u64
    }

    /// Truncating, deterministic distance.
    pub fn distance(self, other: Self) -> u32 {
        integer_sqrt(self.distance_squared(other))
    }

    #[inline]
    pub fn manhattan_distance(self, other: Self) -> u32 {
        self.x.abs_diff(other.x) as u32 + self.y.abs_diff(other.y) as u32
    }

    /// Gets the [`TowerNeighbor`] of `other_id` compared to `self` if it exists. Can get `other_id`
    /// from `self` and `neighbor` with [`Self::neighbor_unchecked`]
    pub fn neighbor_to(self, other_id: TowerId) -> Option<TowerNeighbor> {
        TowerNeighbor::try_from(other_id.0.as_i16vec2() - self.0.as_i16vec2())
            .ok()
            .filter(|&n| NEIGHBOR_TABLE.neighbors(self) & (1 << n as u8) != 0)
    }

    /// Faster than [`neighbor_to`][`Self::neighbor_to`], but assumes that `self` and `other_id` are
    /// neighbors.
    ///
    /// **Panics**
    ///
    /// If `self` and `other_id` are > 1 unit apart in x or y, or if `self == other_id`.
    /// In debug mode if `self` and `other_id` aren't neighbors (no road).
    pub fn neighbor_to_unchecked(self, other_id: TowerId) -> TowerNeighbor {
        debug_assert!(self.is_neighbor(other_id));
        TowerNeighbor::try_from(other_id.0.as_i16vec2() - self.0.as_i16vec2()).unwrap()
    }

    /// Returns the [`TowerId`] of the neighbor at `neighbor` from `self` if it exists.
    pub fn neighbor(self, neighbor: TowerNeighbor) -> Option<TowerId> {
        let other_id = (self.0.as_i16vec2() + I16Vec2::from(neighbor))
            .as_u16vec2()
            .into();
        self.is_neighbor(other_id).then_some(other_id)
    }

    /// Faster than [`neighbor`][`Self::neighbor`], but assumes that the neighbor exists.
    ///
    /// **Panics**
    ///
    /// In debug mode if the neighbor does not exist.
    /// In release mode an arbitrary [`TowerId`] is returned (because of possible overflow).
    pub fn neighbor_unchecked(self, neighbor: TowerNeighbor) -> TowerId {
        let other_id = (self.0.as_i16vec2() + I16Vec2::from(neighbor))
            .as_u16vec2()
            .into();
        debug_assert!(self.is_neighbor(other_id));
        other_id
    }

    /// Iterates a `radius` around the [`TowerId`] in world coordinates (not towers).
    /// Uses floating point arithmetic.
    pub fn iter_radius(self, radius: u16) -> impl Iterator<Item = TowerId> + Clone {
        let center = self.as_vec2();
        let radius_squared = (radius as u32).pow(2) as f32;
        let r = U16Vec2::splat(radius.div_ceil(TowerId::CONVERSION) as u16);

        // TODO clamp to World::SIZE - 1.
        let rect = TowerRectangle::new(
            TowerId(self.0.saturating_sub(r)),
            TowerId(self.0.saturating_add(r)),
        );
        rect.into_iter()
            .filter(move |id| id.as_vec2().distance_squared(center) <= radius_squared)
    }
}

impl Deref for TowerId {
    type Target = U16Vec2;

    fn deref(&self) -> &U16Vec2 {
        &self.0
    }
}

impl DerefMut for TowerId {
    fn deref_mut(&mut self) -> &mut U16Vec2 {
        &mut self.0
    }
}

impl From<U16Vec2> for TowerId {
    fn from(u16vec2: U16Vec2) -> Self {
        Self(u16vec2)
    }
}

impl From<TowerId> for U16Vec2 {
    fn from(tower_id: TowerId) -> Self {
        tower_id.0
    }
}

/// Neighbors of a [`TowerId`] 1 unit away.
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u8)]
pub enum TowerNeighbor {
    N,
    NE,
    E,
    SE,
    S,
    SW,
    W,
    NW,
}

impl TowerNeighbor {
    /// Faster than [`strum::IntoEnumIterator`].
    pub fn iter() -> impl Iterator<Item = Self> {
        (0..8).map(|i| Self::try_from(i).unwrap())
    }

    /// Gets the opposite [`TowerNeighbor`] aka [`TowerNeighbor::N`] returns [`TowerNeighbor::S`].
    pub fn opposite(self) -> Self {
        Self::try_from((self as u8 + 4) & 7).unwrap()
    }
}

impl TryFrom<u8> for TowerNeighbor {
    type Error = ();

    fn try_from(v: u8) -> Result<Self, Self::Error> {
        use TowerNeighbor::*;
        let neighbor = match v {
            0 => N,
            1 => NE,
            2 => E,
            3 => SE,
            4 => S,
            5 => SW,
            6 => W,
            7 => NW,
            _ => return Err(()),
        };
        debug_assert_eq!(neighbor as u8, v);
        Ok(neighbor)
    }
}

impl TryFrom<I16Vec2> for TowerNeighbor {
    type Error = ();

    fn try_from(v: I16Vec2) -> Result<Self, Self::Error> {
        use TowerNeighbor::*;
        Ok(match (v.x, v.y) {
            (0, 1) => N,
            (1, 1) => NE,
            (1, 0) => E,
            (1, -1) => SE,
            (0, -1) => S,
            (-1, -1) => SW,
            (-1, 0) => W,
            (-1, 1) => NW,
            _ => return Err(()),
        })
    }
}

impl From<TowerNeighbor> for I16Vec2 {
    fn from(neighbor: TowerNeighbor) -> Self {
        use TowerNeighbor::*;
        let (x, y) = match neighbor {
            N => (0, 1),
            NE => (1, 1),
            E => (1, 0),
            SE => (1, -1),
            S => (0, -1),
            SW => (-1, -1),
            W => (-1, 0),
            NW => (-1, 1),
        };
        let v = I16Vec2::new(x, y);
        debug_assert_eq!(Self::try_from(v).unwrap(), v);
        v
    }
}

struct OffsetTable {
    offsets: [[u8; WorldChunks::SIZE]; WorldChunks::SIZE],
}
static OFFSET_TABLE: LazyLock<Box<OffsetTable>> = LazyLock::new(OffsetTable::new);
impl OffsetTable {
    fn new() -> Box<Self> {
        let mut me = Box::new(OffsetTable {
            offsets: [[0; WorldChunks::SIZE]; WorldChunks::SIZE],
        });
        for (y, v) in me.offsets.iter_mut().enumerate() {
            for (x, v) in v.iter_mut().enumerate() {
                let mut hash = FNV_OFFSET;
                let mut write_u8 = |u: u8| {
                    hash = hash.wrapping_mul(FNV_PRIME);
                    hash ^= u as u32;
                };
                let mut write_u16 = |u: u16| {
                    let bytes = u.to_le_bytes();
                    write_u8(bytes[0]);
                    write_u8(bytes[1]);
                };
                write_u16(x as u16);
                write_u16(y as u16);

                // Certain fnv bits can be low quality so combine them with xor.
                let hash = condense!(condense!(hash, u16), u8);
                let offsets = U8Vec2::new(hash & 3, (hash >> 4) & 3) + U8Vec2::splat(1);
                *v = offsets.x | (offsets.y << 4);
            }
        }
        me
    }

    fn offset(&self, tower_id: TowerId) -> U16Vec2 {
        // Mask for now to avoid out of bounds (TODO don't create out of bounds TowerIds).
        const MASK: usize = WorldChunks::SIZE - 1;
        const _: () = assert!((MASK + 1).is_power_of_two());
        let offset = self.offsets[tower_id.y as usize & MASK][tower_id.x as usize & MASK];
        U16Vec2::new((offset & 15) as u16, (offset >> 4) as u16)
    }
}

struct NeighborTable {
    neighbors: [[u8; WorldChunks::SIZE]; WorldChunks::SIZE],
}
static NEIGHBOR_TABLE: LazyLock<Box<NeighborTable>> = LazyLock::new(NeighborTable::new);
impl NeighborTable {
    fn new() -> Box<Self> {
        let mut me = Box::new(NeighborTable {
            neighbors: [[0; WorldChunks::SIZE]; WorldChunks::SIZE],
        });

        // This might return Some for towers outside 8 surrounding tower ids, but we only call
        // it on the 8 possible neighbors.
        fn are_neighbors(a: TowerId, b: TowerId) -> bool {
            if a == b || !WorldChunks::RECTANGLE.contains(b) {
                return false; // Same or outside world.
            }
            let distance = a.distance_squared(b);
            if distance > World::MAX_ROAD_LENGTH_SQUARED {
                return false; // Too far apart.
            }
            let diagonal = b.x != a.x && b.y != a.y;
            if diagonal {
                let other1 = TowerId::new(a.x, b.y);
                let other2 = TowerId::new(b.x, a.y);
                let other_distance = other1.distance_squared(other2);
                if other_distance <= distance {
                    return false; // Intersecting a shorter road.
                }
            }
            true
        }

        for (y, v) in me.neighbors.iter_mut().enumerate() {
            for (x, v) in v.iter_mut().enumerate() {
                let tower_id = TowerId::new(x as u16, y as u16);
                let mut bits = 0;

                for n in TowerNeighbor::iter() {
                    let other_id = (tower_id.0.as_i16vec2() + I16Vec2::from(n))
                        .as_u16vec2()
                        .into();

                    let are_neighbors = are_neighbors(tower_id, other_id);
                    bits |= (are_neighbors as u8) << n as u8;
                }
                *v = bits;
            }
        }
        me
    }

    fn neighbors(&self, tower_id: TowerId) -> u8 {
        self.neighbors[tower_id.y as usize][tower_id.x as usize]
    }
}

#[cfg(test)]
mod tests {
    use crate::tower::id::{NeighborTable, OffsetTable, TowerNeighbor};
    use crate::tower::TowerId;
    use crate::world::World;
    use std::hint::black_box;
    use test::Bencher;

    #[test]
    fn test_opposite() {
        fn test(a: TowerNeighbor, b: TowerNeighbor) {
            assert_eq!(a.opposite(), b);
            assert_eq!(b.opposite(), a);
        }
        test(TowerNeighbor::N, TowerNeighbor::S);
        test(TowerNeighbor::NE, TowerNeighbor::SW);
        test(TowerNeighbor::E, TowerNeighbor::W);
        test(TowerNeighbor::SE, TowerNeighbor::NW);
    }

    fn neighbor_pair() -> (TowerId, TowerId) {
        let tower_id = World::CENTER;
        (tower_id, tower_id.neighbors().next().unwrap())
    }

    #[bench]
    fn bench_offset_table(b: &mut Bencher) {
        b.iter(|| OffsetTable::new());
    }

    #[bench]
    fn bench_neighbor_table(b: &mut Bencher) {
        b.iter(|| NeighborTable::new());
    }

    #[bench]
    fn bench_neighbors(b: &mut Bencher) {
        let tower_id = World::CENTER;
        b.iter(|| {
            for other_id in black_box(tower_id).neighbors() {
                black_box(other_id);
            }
        })
    }

    #[test]
    fn test_neighbor_to() {
        let (tower_id, other_id) = neighbor_pair();
        assert!(tower_id.neighbor_to(other_id).is_some());
    }

    #[bench]
    fn bench_neighbor_to(b: &mut Bencher) {
        let (tower_id, other_id) = neighbor_pair();
        b.iter(|| black_box(tower_id).neighbor_to(black_box(other_id)));
    }

    #[test]
    fn test_neighbor_to_unchecked() {
        let (tower_id, other_id) = neighbor_pair();
        tower_id.neighbor_to_unchecked(other_id);
    }

    #[bench]
    fn bench_neighbor_to_unchecked(b: &mut Bencher) {
        let (tower_id, other_id) = neighbor_pair();
        b.iter(|| black_box(tower_id).neighbor_to_unchecked(black_box(other_id)));
    }

    #[test]
    fn test_neighbor() {
        let (a, b) = neighbor_pair();
        assert_eq!(a.neighbor(a.neighbor_to_unchecked(b)), Some(b));
    }

    #[bench]
    fn bench_neighbor(b: &mut Bencher) {
        let (tower_id, other_id) = neighbor_pair();
        let neighbor = tower_id.neighbor_to_unchecked(other_id);
        b.iter(|| black_box(tower_id).neighbor(black_box(neighbor)));
    }

    #[test]
    fn test_neighbor_unchecked() {
        let (a, b) = neighbor_pair();
        assert_eq!(a.neighbor_unchecked(a.neighbor_to_unchecked(b)), b);
    }

    #[bench]
    fn bench_neighbor_unchecked(b: &mut Bencher) {
        let (tower_id, other_id) = neighbor_pair();
        let neighbor = tower_id.neighbor_to_unchecked(other_id);
        b.iter(|| black_box(tower_id).neighbor_unchecked(black_box(neighbor)));
    }
}
