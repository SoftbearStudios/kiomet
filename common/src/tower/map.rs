// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::tower::{TowerId, TowerRectangle};

/// Like a `HashMap<TowerId, T>` but dense instead of sparse and requires a bounding rectangle.
#[derive(Clone)]
pub struct TowerMap<T> {
    data: Vec<Option<T>>,
    bounds: TowerRectangle,
    len: u32,
}

impl<T> Default for TowerMap<T> {
    fn default() -> Self {
        Self {
            data: vec![],
            bounds: TowerRectangle::invalid(),
            len: 0,
        }
    }
}

impl<T: PartialEq> PartialEq<TowerMap<T>> for TowerMap<T> {
    fn eq(&self, other: &Self) -> bool {
        // TODO pick specified order of iter.
        Iterator::eq(self.iter(), other.iter())
    }
}

impl<T> TowerMap<T> {
    pub fn with_bounds(bounds: TowerRectangle) -> Self {
        let mut ret = Self::default();
        ret.reset_bounds(bounds);
        ret
    }

    /// Sets the bounds of the map and clears it.
    pub fn reset_bounds(&mut self, bounds: TowerRectangle) {
        self.bounds = bounds;
        self.clear();
    }

    /// Gets the bounds of the map.
    pub fn bounds(&self) -> TowerRectangle {
        self.bounds
    }

    /// Clears the map without affecting its bounds.
    pub fn clear(&mut self) {
        self.data.clear();
        self.data.resize_with(self.bounds.area() as usize, || None);
        self.len = 0;
    }

    /// Returns if the set contains a TowerId.
    pub fn contains(&self, tower_id: TowerId) -> bool {
        self.get(tower_id).is_some()
    }

    pub fn get(&self, tower_id: TowerId) -> Option<&T> {
        self.index(tower_id).and_then(|o| self.data[o].as_ref())
    }

    pub fn get_mut(&mut self, tower_id: TowerId) -> Option<&mut T> {
        self.index(tower_id).and_then(|o| self.data[o].as_mut())
    }

    fn index(&self, tower_id: TowerId) -> Option<usize> {
        if self.bounds.contains(tower_id) {
            let relative = tower_id.0 - self.bounds.bottom_left.0;
            Some(relative.x as usize + relative.y as usize * self.bounds.dimensions().x as usize)
        } else {
            None
        }
    }

    /// Inserts a TowerId into the map. Returns the previous value if it existed.
    /// # Panics
    /// If the TowerId is out of bounds of the last reset_bounds operation.
    pub fn insert(&mut self, tower_id: TowerId, value: T) -> Option<T> {
        let index = self.index(tower_id).unwrap_or_else(|| {
            panic!(
                "index out of bounds: the bounds are {:?} but the index is {:?}",
                self.bounds, tower_id
            )
        });
        let ret = std::mem::replace(&mut self.data[index], Some(value));
        if ret.is_none() {
            self.len += 1;
        }
        ret
    }

    /// Returns true if the length of the map is zero.
    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Iterates the map in an unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = (TowerId, &T)> + '_ {
        self.bounds
            .into_iter()
            .filter_map(|id| self.get(id).map(|i| (id, i)))
    }

    /// Returns the length of the set. This operation is O(1).
    pub fn len(&self) -> usize {
        self.len as usize
    }

    /// Removes a TowerId from the map. Returns the value if it existed.
    pub fn remove(&mut self, tower_id: TowerId) -> Option<T> {
        self.index(tower_id).and_then(|index| {
            let ret = std::mem::replace(&mut self.data[index], None);
            if ret.is_some() {
                self.len -= 1;
            }
            ret
        })
    }
}
