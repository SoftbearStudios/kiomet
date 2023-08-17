// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use super::{Chunk, RelativeTowerId};
use crate::{info::OnInfo, world::Apply};
use common_util::actor2::Message;
use core_protocol::prelude::*;

#[derive(Clone, Copy, Debug, Encode, Decode)]
pub enum ChunkHaltEvent {
    Force(RelativeTowerId, #[bitcode_hint(gamma)] u32),
    SupplyLine(RelativeTowerId),
}

impl Message for ChunkHaltEvent {}

impl<C: OnInfo> Apply<ChunkHaltEvent, C> for Chunk {
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
