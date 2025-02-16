// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::ChunkId;
use crate::chunk::{Chunk, RelativeTowerId};
use crate::info::*;
use crate::unit::Unit;
use crate::world::Apply;
use kodiak_common::actor_model::*;
use kodiak_common::bitcode::{self, *};
use kodiak_common::PlayerId;

/// The first input that runs each tick. Things that can't be done properly while `ChunkEvent`s are
/// in flight.
#[derive(Clone, Debug, Encode, Decode)]
pub enum ChunkMaintenance {
    /// If `ChunkEvent`s are in flight, might destroy `Tower` that has incoming units.
    Destroy { tower_ids: Vec<RelativeTowerId> },
    /// If `ChunkEvent`s are in flight with units of `player_id` this won't kill them.
    KillPlayer { player_id: PlayerId },
}

impl Message for ChunkMaintenance {}

impl Apply<ChunkMaintenance, Dst<'_, ChunkId, OnInfo<'_>>> for Chunk {
    fn apply(&mut self, u: &ChunkMaintenance, context: &mut Dst<'_, ChunkId, OnInfo<'_>>) {
        match u.clone() {
            ChunkMaintenance::Destroy { tower_ids } => {
                for tower_id in tower_ids {
                    let tower = self.remove(tower_id);
                    debug_assert!(tower.can_destroy());
                }
            }
            ChunkMaintenance::KillPlayer { player_id } => {
                for (tower_id, tower) in self.iter_mut(self.chunk_id) {
                    if tower.player_id == Some(player_id) {
                        tower.units.subtract(Unit::Ruler, usize::MAX);
                        tower.units.subtract(Unit::Shield, usize::MAX);
                        tower.set_player_id(None);

                        // Don't trigger LostRulerEvents.
                        context(InfoEvent {
                            position: tower_id.as_vec2(),
                            info: Info::LostTower {
                                tower_id,
                                player_id,
                                reason: LostTowerReason::PlayerKilled,
                            },
                        });
                    }
                    tower
                        .inbound_forces
                        .retain(|force| force.player_id != Some(player_id));
                    tower
                        .outbound_forces
                        .retain(|force| force.player_id != Some(player_id));
                }
            }
        }
    }
}
