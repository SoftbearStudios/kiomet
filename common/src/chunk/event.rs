// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chunk::{Chunk, ChunkId, RelativeTowerId};
use crate::force::{Force, Path};
use crate::info::*;
use crate::tower::Tower;
use crate::tower::TowerType;
use crate::unit::Unit;
use crate::units::Units;
use crate::world::Apply;
use kodiak_common::actor_model::*;
use kodiak_common::bitcode::{self, *};
use kodiak_common::{define_on, PlayerId, RankNumber};
use std::num::NonZeroU8;

/// A [`ChunkEvent`] with it's destination [`ChunkId`].
pub struct AddressedChunkEvent {
    pub dst: ChunkId,
    pub event: ChunkEvent,
}

/*
// TODO generic OnEvent trait.
pub trait OnChunkEvent {
    fn on_chunk_event(&mut self, src: ChunkId, event: AddressedChunkEvent);
    fn on_chunk_events(
        &mut self,
        src: ChunkId,
        events: impl IntoIterator<Item = AddressedChunkEvent>,
    ) {
        for event in events {
            self.on_chunk_event(src, event)
        }
    }
}
*/

impl Tower {
    // TODO move?
    #[must_use]
    pub fn deploy_force(&mut self, path: Path) -> [AddressedChunkEvent; 2] {
        #[cfg(debug_assertions)]
        let had = self.units.clone();

        let units = self.take_force_units();
        let player_id = self.player_id.unwrap();
        if units.is_empty() {
            #[cfg(debug_assertions)]
            debug_assert!(
                false,
                "inefficient: empty force in deploy force (tower had {:?})",
                had
            );
        }

        self.send_force(Force::new(player_id, units, path))
    }

    #[must_use]
    fn send_force(&mut self, force: Force) -> [AddressedChunkEvent; 2] {
        let outbound = {
            let (dst, tower_id) = force.current_source().split();
            AddressedChunkEvent {
                dst,
                event: ChunkEvent::add_outbound_force(tower_id, &force),
            }
        };

        let (dst, tower_id) = force.current_destination().split();
        let inbound = AddressedChunkEvent {
            dst,
            event: ChunkEvent::AddInboundForce { tower_id, force },
        };
        [outbound, inbound]
    }
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum ChunkInput {
    // Only used for debugging with chonk.
    AddInboundForce {
        tower_id: RelativeTowerId,
        force: Force,
    },
    /// Useful to make some space while spawning. Implied for the spawn tower id.
    ClearZombies { tower_id: RelativeTowerId },
    DeployForce {
        tower_id: RelativeTowerId,
        path: Path,
    },
    Generate {
        tower_ids: Vec<RelativeTowerId>, // TODO RelativeTowerIdSet
    },
    SetSupplyLine {
        tower_id: RelativeTowerId,
        path: Option<Path>,
    },
    Spawn {
        tower_id: RelativeTowerId,
        player_id: PlayerId,
        rank: Option<RankNumber>,
    },
    UpgradeTower {
        tower_id: RelativeTowerId,
        tower_type: TowerType,
    },
}

impl Message for ChunkInput {}

impl Apply<ChunkInput, OnChunkEvent<'_, OnInfo<'_>>> for Chunk {
    fn apply(&mut self, u: &ChunkInput, context: &mut OnChunkEvent<'_, OnInfo<'_>>) {
        match u.clone() {
            ChunkInput::AddInboundForce { tower_id, force } => {
                self[tower_id].inbound_forces.push(force);
            }
            ChunkInput::ClearZombies { tower_id } => {
                let tower = &mut self[tower_id];
                if tower.player_id.is_none() {
                    debug_assert!(!tower.units.has_ruler());
                    tower.units.clear();
                }
            }
            ChunkInput::DeployForce { tower_id, path } => {
                for chunk_event in self[tower_id].deploy_force(path) {
                    context.on_chunk_event(self.chunk_id, chunk_event.dst, chunk_event.event);
                }
            }
            ChunkInput::Generate { tower_ids } => {
                for tower_id in tower_ids {
                    self.insert(tower_id, Tower::new(tower_id.upgrade(self.chunk_id)));
                }
            }
            ChunkInput::SetSupplyLine { tower_id, path } => self[tower_id].supply_line = path,
            ChunkInput::Spawn {
                tower_id,
                player_id,
                rank,
            } => {
                let chunk_id = self.chunk_id;
                let tower = &mut self[tower_id];
                let tower_id = tower_id.upgrade(chunk_id);

                debug_assert_eq!(tower.player_id, None, "{:?}", player_id);
                debug_assert!(!tower.units.has_ruler());

                tower.units = Units::default();
                tower.set_player_id(Some(player_id));

                context(InfoEvent {
                    info: Info::GainedTower {
                        player_id,
                        tower_id,
                        reason: GainedTowerReason::Spawned,
                    },
                    position: tower_id.as_vec2(),
                });

                tower
                    .units
                    .add_to_tower(Unit::Ruler, 1, tower.tower_type, false);
                tower
                    .units
                    .add_to_tower(Unit::Shield, usize::MAX, tower.tower_type, false);

                for unit in [Unit::Soldier, Unit::Fighter] {
                    let mut soldiers = Units::default();
                    soldiers.add(
                        unit,
                        if unit == Unit::Fighter {
                            2
                        } else if rank >= Some(RankNumber::Rank2) {
                            8
                        } else {
                            4
                        },
                    );
                    soldiers.add(Unit::Shield, 15);
                    for neighbor in tower_id.neighbors() {
                        let force = Force::new(
                            player_id,
                            soldiers.clone(),
                            Path::new(vec![tower_id, neighbor]),
                        );
                        for chunk_input in tower.send_force(force) {
                            context.on_chunk_event(chunk_id, chunk_input.dst, chunk_input.event);
                        }
                    }
                    if rank < Some(RankNumber::Rank5) {
                        // Skip fighters.
                        break;
                    }
                }
            }
            ChunkInput::UpgradeTower {
                tower_id,
                tower_type,
            } => {
                let tower = &mut self[tower_id];
                tower.tower_type = tower_type;

                // The upgrade will temporarily suspend this tower.
                tower.delay = NonZeroU8::new(tower_type.delay().0.try_into().unwrap());

                // The new tower may have different unit capacities.
                tower.reconcile_units();

                if tower.supply_line.is_some() && !tower.generates_mobile_units() {
                    tower.supply_line = None;
                }
            }
        }
    }
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum ChunkEvent {
    AddInboundForce {
        tower_id: RelativeTowerId,
        force: Force,
    },
    /// For adding the shadow force (to allow inter-tower battles).
    AddOutboundForce {
        tower_id: RelativeTowerId,
        force: Force,
    },
}

impl Message for ChunkEvent {}
define_on!(ChunkId, ChunkId, ChunkEvent);

impl<C: ?Sized> Apply<ChunkEvent, C> for Chunk {
    fn apply(&mut self, u: &ChunkEvent, _context: &mut C) {
        match u.clone() {
            ChunkEvent::AddInboundForce { tower_id, force } => {
                #[cfg(debug_assertions)]
                if self.get(tower_id).is_none() {
                    panic!("missing dst, {tower_id:?}, {force:?}");
                }
                self[tower_id].inbound_forces.push(force)
            }
            ChunkEvent::AddOutboundForce { tower_id, force } => {
                self[tower_id].outbound_forces.push(force)
            }
        }
    }
}

impl ChunkEvent {
    /// More efficient than creating [`Self::AddOutboundForce`] directly.
    pub fn add_outbound_force(tower_id: RelativeTowerId, force: &Force) -> Self {
        Self::AddOutboundForce {
            tower_id,
            force: force.halted(), // bandwidth optimization: Outbound forces don't need whole path.
        }
    }
}
