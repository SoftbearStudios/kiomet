// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Chunk, RelativeTowerId};
use crate::world::Apply;
use kodiak_common::actor_model::Message;
use kodiak_common::bitcode::{self, *};

#[derive(Clone, Copy, Debug, Encode, Decode)]
pub enum ChunkHaltEvent {
    Force(RelativeTowerId, u32),
    SupplyLine(RelativeTowerId),
}

impl Message for ChunkHaltEvent {}

impl<C: ?Sized> Apply<ChunkHaltEvent, C> for Chunk {
    fn apply(&mut self, u: &ChunkHaltEvent, _context: &mut C) {
        match *u {
            ChunkHaltEvent::Force(relative_tower_id, index) => {
                self[relative_tower_id].inbound_forces[index as usize].halt();
            }
            ChunkHaltEvent::SupplyLine(relative_tower_id) => {
                self[relative_tower_id].supply_line = None;
            }
        }
    }
}
