// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::tower::id::TowerNeighbor;
use crate::tower::TowerId;
use crate::world::{World, WorldChunks};
use std::collections::VecDeque;
use std::sync::LazyLock;

static CONNECTIVITY_TABLE: LazyLock<Box<ConnectivityTable>> = LazyLock::new(ConnectivityTable::new);

struct ConnectivityTable([[Option<TowerNeighbor>; WorldChunks::SIZE]; WorldChunks::SIZE]);

impl ConnectivityTable {
    fn new() -> Box<Self> {
        let mut me = Box::new(Self([[None; WorldChunks::SIZE]; WorldChunks::SIZE]));
        let mut frontier = VecDeque::with_capacity(2048);

        // The world center connects to one of its neighbors arbitrarily for simplicity.
        let tower_id = World::CENTER;
        let neighbor = tower_id.neighbor_to_unchecked(tower_id.neighbors().next().unwrap());
        *me.get_mut(tower_id) = Some(neighbor);
        frontier.push_back(tower_id);

        while let Some(parent_id) = frontier.pop_front() {
            for (neighbor, tower_id) in parent_id.neighbors_enumerated() {
                let v = me.get_mut(tower_id);
                if v.is_some() {
                    continue;
                }

                *v = Some(neighbor.opposite());
                frontier.push_back(tower_id);
            }
        }

        me
    }

    fn get(&self, tower_id: TowerId) -> Option<TowerNeighbor> {
        self.0[tower_id.y as usize][tower_id.x as usize]
    }

    fn get_mut(&mut self, tower_id: TowerId) -> &mut Option<TowerNeighbor> {
        &mut self.0[tower_id.y as usize][tower_id.x as usize]
    }
}

impl TowerId {
    pub fn connectivity(self) -> Option<TowerNeighbor> {
        CONNECTIVITY_TABLE.get(self)
    }

    pub fn connectivity_id(self) -> Option<TowerId> {
        Some(self.neighbor_unchecked(self.connectivity()?))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use kodiak_common::x_vec2::I16Vec2;
    use std::collections::HashSet;
    use test::Bencher;

    #[test]
    fn test_connecitivity() {
        use kodiak_common::rand::prelude::*;
        let mut rng = thread_rng();

        for _ in 0..1000 {
            let mut tower_id = TowerId(
                (World::CENTER.0.as_i16vec2()
                    + I16Vec2::new(rng.gen_range(-100..100), rng.gen_range(-100..100)))
                .as_u16vec2(),
            );
            if tower_id.connectivity().is_none() {
                continue;
            }

            let mut tower_ids = HashSet::new();
            loop {
                if !tower_ids.insert(tower_id) {
                    panic!("cycle {} {tower_id:?}", tower_ids.len());
                }

                if tower_id == World::CENTER {
                    break; // We've reached the center.
                }

                let towards_center = tower_id.connectivity().unwrap();
                println!("test {towards_center:?}");
                tower_id = tower_id.neighbor_unchecked(towards_center);
            }
        }
    }

    #[bench]
    fn bench_connectivity_table(b: &mut Bencher) {
        b.iter(|| ConnectivityTable::new());
    }
}
