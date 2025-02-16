// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::tower::map::TowerMap;
use crate::tower::{TowerId, TowerRectangle};

/// Like a `HashSet<TowerId>` but dense instead of sparse and
/// requires a bounding rectangle.
#[derive(Clone, Default)]
pub struct TowerSet(TowerMap<()>);

impl TowerSet {
    pub fn with_bounds(bounds: TowerRectangle) -> Self {
        Self(TowerMap::with_bounds(bounds))
    }

    /// Inserts a TowerId into the map. Returns true if the TowerId was not present.
    /// # Panics
    /// If the TowerId is out of bounds of the last reset_bounds operation.
    pub fn insert(&mut self, tower_id: TowerId) -> bool {
        self.0.insert(tower_id, ()).is_none()
    }

    /// Removes a TowerId from the set. Returns true if the TowerId was present.
    pub fn remove(&mut self, tower_id: TowerId) -> bool {
        self.0.remove(tower_id).is_some()
    }

    /// Returns whether set contains the [`TowerId`].
    pub fn contains(&self, tower_id: TowerId) -> bool {
        self.0.contains(tower_id)
    }

    /// Returns number of items in set.
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Returns true if the length of the set is zero.
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    /// Clears the set without affecting its bounds.
    pub fn clear(&mut self) {
        self.0.clear()
    }

    /// Sets the bounds of the set and clears it.
    pub fn reset_bounds(&mut self, bounds: TowerRectangle) {
        self.0.reset_bounds(bounds)
    }

    /// Iterates the set in an unspecified order.
    pub fn iter(&self) -> impl Iterator<Item = TowerId> + '_ {
        self.0.iter().map(|(id, _)| id)
    }
}

#[cfg(test)]
mod tests {
    use crate::tower::{TowerId, TowerRectangle, TowerSet};
    use kodiak_common::x_vec2::U16Vec2;
    use std::collections::HashSet;

    #[test]
    fn tower_set() {
        let bounds = TowerRectangle::new_centered(TowerId::new(100, 100), U16Vec2::new(100, 100));
        let mut set = TowerSet::default();
        set.reset_bounds(bounds);
        assert_eq!(set.iter().collect::<Vec<_>>(), []);
        assert_eq!(set.len(), 0);

        let one = TowerId::new(125, 125);
        set.insert(one);
        assert_eq!(set.iter().collect::<Vec<_>>(), [one]);
        assert_eq!(set.len(), 1);
        assert!(set.contains(one));
        assert!(!set.insert(one));

        let two = TowerId::new(130, 130);
        assert!(set.insert(two));
        assert_eq!(set.len(), 2);

        let three = TowerId::new(0, 0);
        assert!(!set.contains(three));
        assert!(!set.remove(three));

        assert!(set.remove(one));
        assert_eq!(set.len(), 1);
        assert!(!set.remove(one));
        assert!(!set.contains(one));

        set.clear();
        assert_eq!(set.iter().collect::<Vec<_>>(), []);
        assert_eq!(set.len(), 0);
        assert!(set.is_empty());

        let bounds = TowerRectangle::new(TowerId::new(0, 0), TowerId::new(200, 200));
        set.reset_bounds(bounds);

        assert!(set.insert(one));
        assert!(set.insert(two));
        assert!(set.insert(three));

        assert_eq!(set.len(), 3);
        assert!(!set.is_empty());
        assert_eq!(set.iter().collect::<HashSet<_>>(), [one, two, three].into());

        set.clear();
        assert!(!set.contains(three));
    }
}
