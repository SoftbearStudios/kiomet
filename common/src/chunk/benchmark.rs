// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Chunk, ChunkId, RelativeTowerId};
use crate::{
    force::{Force, Path},
    info::InfoEvent,
    player::Player,
    singleton::Singleton,
    tower::Tower,
    units::Units,
};
use kodiak_common::rand::{thread_rng, Rng};
use kodiak_common::PlayerId;
use std::num::NonZeroU32;
use test::Bencher;

#[bench]
fn bench(b: &mut Bencher) {
    let chunk_id = ChunkId::new(5, 5);
    let mut chunk = Chunk::new(chunk_id);
    for i in 0..=u8::MAX {
        let relative_tower_id = RelativeTowerId(i);
        chunk.insert(
            relative_tower_id,
            Tower::new(relative_tower_id.upgrade(chunk_id)),
        );
    }
    let mut paths = Vec::new();
    for (tower_id, _) in chunk.iter(chunk_id) {
        for neighbor_tower_id in tower_id.neighbors() {
            if neighbor_tower_id.split().0 == chunk_id {
                paths.push((tower_id, neighbor_tower_id));
            }
        }
    }
    let mut rng = thread_rng();
    for (src, dst) in paths {
        for _ in 0..rng.gen_range(0..8) {
            let mut force = Force::new(
                PlayerId(NonZeroU32::new(rng.gen_range(1..=2)).unwrap()),
                Units::random_units(rng.gen_range(1..32), false, rng.gen()),
                Path::new(vec![src, dst]),
            );
            force.path_progress = rng.gen_range(0..10);
            chunk[src.split().1].outbound_forces.push(force.clone());
            chunk[dst.split().1].inbound_forces.push(force);
        }
    }

    let singleton = Singleton::default();
    let player = Player {
        allies: Default::default(),
        new_alliances: Default::default(),
    };

    b.iter(|| {
        let mut chunk = chunk.clone();
        for _ in 0..10 {
            chunk.tick(
                chunk_id,
                |_| &player,
                &singleton,
                |_, _| {},
                &mut |_: InfoEvent| {},
            )
        }
        chunk
    });
}
