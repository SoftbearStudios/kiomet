// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chunk::ChunkRectangle;
use crate::tower::TowerId;
use crate::world::WorldChunks;
use kodiak_common::bitcode::{self, *};
use kodiak_common::U16Vec2;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub struct TowerRectangle {
    pub bottom_left: TowerId,
    pub top_right: TowerId,
}

impl Default for TowerRectangle {
    fn default() -> Self {
        Self::invalid()
    }
}

impl TowerRectangle {
    pub const fn new(bottom_left: TowerId, top_right: TowerId) -> Self {
        Self {
            bottom_left,
            top_right,
        }
    }

    /// Returns a TowerRectangle centered on a center and with certain dimensions.
    /// # Panics
    /// If the rectangle is out of bounds of valid TowerIds.
    pub fn new_centered(center: TowerId, dimensions: U16Vec2) -> Self {
        let mut bottom_left = U16Vec2::ZERO;
        let mut top_right = U16Vec2::ZERO;
        if dimensions.x != 0 && dimensions.y != 0 {
            bottom_left = center
                .0
                .checked_sub(dimensions / 2)
                .expect("bottom left is out of bounds");
            top_right = center
                .0
                .checked_add((dimensions + 1) / 2 - 1)
                .expect("top right is out of bounds");
        }
        Self::new(bottom_left.into(), top_right.into())
    }

    /// Returns the dimensions (width, height) of the tower rectangle.
    pub fn dimensions(self) -> U16Vec2 {
        self.top_right.0 - self.bottom_left.0 + 1
    }

    /// Returns the area of the rectangle (width * height).
    pub fn area(self) -> u32 {
        if self.is_valid() {
            let dimensions = self.dimensions();
            dimensions.x as u32 * dimensions.y as u32
        } else {
            0
        }
    }

    /// Returns one potential invalid [`TowerRectangle`].
    // TODO only allow 1 invalid rect?
    pub fn invalid() -> Self {
        Self {
            bottom_left: TowerId::new(WorldChunks::SIZE as u16 - 1, WorldChunks::SIZE as u16 - 1),
            top_right: TowerId::new(0, 0),
        }
    }

    pub fn bounding<I: IntoIterator<Item = TowerId>>(tower_ids: I) -> Self {
        let mut ret = Self {
            bottom_left: TowerId::new(u16::MAX, u16::MAX),
            top_right: TowerId::new(0, 0),
        };

        debug_assert!(!ret.is_valid());

        for tower_id in tower_ids.into_iter() {
            ret.bottom_left.x = ret.bottom_left.x.min(tower_id.x);
            ret.bottom_left.y = ret.bottom_left.y.min(tower_id.y);
            ret.top_right.x = ret.top_right.x.max(tower_id.x);
            ret.top_right.y = ret.top_right.y.max(tower_id.y);
            debug_assert!(ret.is_valid());
            debug_assert!(ret.contains(tower_id));
        }

        ret
    }

    #[must_use = "returns the mutated rectangle"]
    pub fn add_margin(mut self, margin: u16) -> Self {
        if self.is_valid() {
            self.bottom_left.x = self.bottom_left.x.saturating_sub(margin);
            self.bottom_left.y = self.bottom_left.y.saturating_sub(margin);
            self.top_right.x = self.top_right.x.saturating_add(margin);
            self.top_right.y = self.top_right.y.saturating_add(margin);
            self
        } else {
            // Never become valid.
            self
        }
    }

    /// Return a new rectangle that is not in (valid) excess of other.
    pub fn clamp_to(self, other: Self) -> Self {
        Self {
            bottom_left: TowerId::new(
                self.bottom_left.x.max(other.bottom_left.x),
                self.bottom_left.y.max(other.bottom_left.y),
            ),
            top_right: TowerId::new(
                self.top_right.x.min(other.top_right.x),
                self.top_right.y.min(other.top_right.y),
            ),
        }
    }

    pub fn is_valid(self) -> bool {
        self.top_right.x >= self.bottom_left.x && self.top_right.y >= self.bottom_left.y
    }

    pub fn contains(self, tower_id: TowerId) -> bool {
        tower_id.x >= self.bottom_left.x
            && tower_id.y >= self.bottom_left.y
            && tower_id.x <= self.top_right.x
            && tower_id.y <= self.top_right.y
    }

    pub fn union(self, other: Self) -> Self {
        if !self.is_valid() {
            return other;
        }
        if !other.is_valid() {
            return self;
        }
        Self {
            bottom_left: TowerId(self.bottom_left.0.min_components(other.bottom_left.0)),
            top_right: TowerId(self.top_right.0.max_components(other.top_right.0)),
        }
    }
}

impl IntoIterator for TowerRectangle {
    type Item = TowerId;
    type IntoIter = impl Iterator<Item = Self::Item> + Clone + 'static;

    fn into_iter(self) -> Self::IntoIter {
        (self.bottom_left.y..=self.top_right.y).flat_map(move |y| {
            (self.bottom_left.x..=self.top_right.x).map(move |x| TowerId::new(x, y))
        })
    }
}

impl From<ChunkRectangle> for TowerRectangle {
    fn from(chunk_rectangle: ChunkRectangle) -> Self {
        Self {
            bottom_left: chunk_rectangle.bottom_left.bottom_left(),
            top_right: chunk_rectangle.top_right.top_right(),
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::chunk::ChunkRectangle;
    use crate::tower::id::TowerId;
    use crate::tower::rectangle::TowerRectangle;

    #[test]
    fn tower_rectangle() {
        let valid_rect = TowerRectangle::new(TowerId::new(1, 2), TowerId::new(3, 4));
        assert!(valid_rect.is_valid());
        assert_eq!(valid_rect.area(), 9);
        assert!(valid_rect.contains(TowerId::new(2, 3)));
        assert!(!valid_rect.contains(TowerId::new(1, 1)));

        let invalid_rect = TowerRectangle::invalid();
        assert!(!invalid_rect.is_valid());
        assert_eq!(TowerRectangle::invalid().area(), 0);
        assert!(!valid_rect.contains(TowerId::new(1, 5)));
        assert!(!valid_rect.contains(TowerId::new(0, 1)));

        let chunk_rect: ChunkRectangle = TowerRectangle::invalid().into();
        assert!(!chunk_rect.is_valid());
    }

    #[test]
    fn union() {
        let a = TowerRectangle::new(TowerId::new(1, 1), TowerId::new(3, 3));
        let b = TowerRectangle::new(TowerId::new(2, 2), TowerId::new(4, 4));
        let c = TowerRectangle::new(TowerId::new(1, 1), TowerId::new(4, 4));
        assert_eq!(a.union(b), c);
    }
}
