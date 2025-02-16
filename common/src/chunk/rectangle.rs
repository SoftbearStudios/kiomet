// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chunk::ChunkId;
use crate::tower::TowerRectangle;
use kodiak_common::bitcode::{self, *};
use kodiak_common::U8Vec2;

#[derive(Copy, Clone, Debug, Eq, PartialEq, Encode, Decode)]
pub struct ChunkRectangle {
    pub bottom_left: ChunkId,
    pub top_right: ChunkId,
}

impl Default for ChunkRectangle {
    fn default() -> Self {
        Self::invalid()
    }
}

impl ChunkRectangle {
    pub fn new(bottom_left: ChunkId, top_right: ChunkId) -> Self {
        Self {
            bottom_left,
            top_right,
        }
    }

    /// Returns one potential invalid [`TowerRectangle`].
    pub fn invalid() -> Self {
        Self {
            bottom_left: ChunkId::new(u8::MAX, u8::MAX),
            top_right: ChunkId::new(0, 0),
        }
    }

    pub fn is_valid(self) -> bool {
        self.top_right.x >= self.bottom_left.x && self.top_right.y >= self.bottom_left.y
    }

    /// Returns the dimensions (width, height) of the tower rectangle.
    pub fn dimensions(self) -> U8Vec2 {
        self.top_right.0 - self.bottom_left.0 + 1
    }

    pub fn contains(self, chunk_id: ChunkId) -> bool {
        chunk_id.x >= self.bottom_left.x
            && chunk_id.y >= self.bottom_left.y
            && chunk_id.x <= self.top_right.x
            && chunk_id.y <= self.top_right.y
    }

    /// Return a new rectangle that is not in (valid) excess of other.
    pub fn clamp_to(self, other: Self) -> Self {
        Self {
            bottom_left: ChunkId::new(
                self.bottom_left.x.max(other.bottom_left.x),
                self.bottom_left.y.max(other.bottom_left.y),
            ),
            top_right: ChunkId::new(
                self.top_right.x.min(other.top_right.x),
                self.top_right.y.min(other.top_right.y),
            ),
        }
    }
}

impl IntoIterator for ChunkRectangle {
    type Item = ChunkId;
    type IntoIter = impl Iterator<Item = Self::Item> + Clone + 'static;

    fn into_iter(self) -> Self::IntoIter {
        (self.bottom_left.y..=self.top_right.y).flat_map(move |y| {
            (self.bottom_left.x..=self.top_right.x).map(move |x| ChunkId::new(x, y))
        })
    }
}

impl From<TowerRectangle> for ChunkRectangle {
    fn from(tower_rectangle: TowerRectangle) -> Self {
        Self::new(
            tower_rectangle.bottom_left.into(),
            tower_rectangle.top_right.into(),
        )
    }
}

#[cfg(test)]
mod tests {
    use crate::chunk::ChunkRectangle;

    #[test]
    fn invalid() {
        assert!(!ChunkRectangle::invalid().is_valid());
    }
}
