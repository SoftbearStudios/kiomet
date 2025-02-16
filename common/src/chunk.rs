// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::combatants::{CombatInfo, CombatSide, Combatants};
use crate::info::{GainedTowerReason, Info, InfoEvent, LostRulerReason, LostTowerReason, OnInfo};
use crate::player::Player;
use crate::shrink_vec;
use crate::singleton::Singleton;
use crate::ticks::Ticks;
use crate::tower::Tower;
use crate::tower::TowerId;
use crate::unit::Unit;
use kodiak_common::bitcode::{self, *};
use kodiak_common::{PlayerId, TicksRepr};
use std::num::NonZeroU8;
use std::ops::{Index, IndexMut};

#[cfg(test)]
mod benchmark;
mod event;
mod halt;
mod id;
mod maintenance;
mod rectangle;

pub use event::*;
pub use halt::ChunkHaltEvent;
pub use id::{ChunkId, RelativeTowerId};
pub use maintenance::ChunkMaintenance;
pub use rectangle::ChunkRectangle;

#[derive(Clone, Debug, Hash, PartialEq, Eq, Encode, Decode)]
pub struct Chunk {
    towers: Box<[Option<Tower>; Self::AREA]>,
    pub(crate) chunk_id: ChunkId, // Temporary hack to get chunk_id inside Apply. TODO C: WithId<ChunkId>.
}

impl Index<RelativeTowerId> for Chunk {
    type Output = Tower;
    fn index(&self, tower_id: RelativeTowerId) -> &Self::Output {
        self.towers[tower_id.0 as usize].as_ref().unwrap()
    }
}

impl IndexMut<RelativeTowerId> for Chunk {
    fn index_mut(&mut self, tower_id: RelativeTowerId) -> &mut Self::Output {
        self.towers[tower_id.0 as usize].as_mut().unwrap()
    }
}

impl Chunk {
    pub const SIZE: usize = 16;
    pub const AREA: usize = Self::SIZE * Self::SIZE;

    #[cfg(feature = "server")]
    pub fn new(chunk_id: ChunkId) -> Self {
        Self {
            towers: Box::new([(); Self::AREA].map(|_| None)),
            chunk_id,
        }
    }

    pub fn get(&self, tower_id: RelativeTowerId) -> Option<&Tower> {
        self.towers[tower_id.0 as usize].as_ref()
    }

    /// Inserts a new [`Tower`].
    ///
    /// **Panics**
    ///
    /// If there is already a [`Tower`] at `tower_id`.
    pub fn insert(&mut self, tower_id: RelativeTowerId, tower: Tower) {
        let old = std::mem::replace(&mut self.towers[tower_id.0 as usize], Some(tower));
        assert!(old.is_none());
    }

    /// Removes the [`Tower`] at `tower_id`.
    ///
    /// **Panics**
    ///
    /// If the [`Tower`] does not exist.
    pub fn remove(&mut self, tower_id: RelativeTowerId) -> Tower {
        std::mem::take(&mut self.towers[tower_id.0 as usize]).unwrap()
    }

    pub fn iter(&self, chunk_id: ChunkId) -> impl Iterator<Item = (TowerId, &Tower)> + Clone {
        self.towers.iter().enumerate().filter_map(move |(i, t)| {
            t.as_ref()
                .map(|t| (RelativeTowerId(i as u8).upgrade(chunk_id), t))
        })
    }

    pub fn iter_mut(&mut self, chunk_id: ChunkId) -> impl Iterator<Item = (TowerId, &mut Tower)> {
        self.towers
            .iter_mut()
            .enumerate()
            .filter_map(move |(i, t)| {
                t.as_mut()
                    .map(|t| (RelativeTowerId(i as u8).upgrade(chunk_id), t))
            })
    }

    pub fn iter_player_ids(&self) -> impl Iterator<Item = PlayerId> + '_ {
        // TODO cache.
        self.towers.iter().flatten().flat_map(|t| {
            t.player_id
                .into_iter()
                .chain(t.inbound_forces.iter().filter_map(|f| f.player_id))
                .chain(t.outbound_forces.iter().filter_map(|f| f.player_id))
        })
    }

    pub fn tick<'a>(
        &mut self,
        chunk_id: ChunkId,
        players: impl Fn(PlayerId) -> &'a Player,
        singleton: &Singleton,
        mut on_event: impl FnMut(ChunkId, ChunkEvent), // TODO put on context?
        context: &mut OnInfo<'_>,
    ) {
        if cfg!(debug_assertions) {
            // Ensure we have all the players, to prevent client crashes.
            for id in self.iter_player_ids() {
                players(id);
            }
        }

        let relationship = |a: Option<PlayerId>, b: Option<PlayerId>| -> Relationship {
            if a == b {
                Relationship::Comrade
            } else if a
                .zip(b)
                .map(|(a, b)| players(a).allies.contains(&b) && players(b).allies.contains(&a))
                .unwrap_or(false)
            {
                Relationship::Ally
            } else {
                Relationship::Enemy
            }
        };

        // TODO better random tick offset (maybe per tower).
        let tick_offset = Ticks::from_repr(u16::from_le_bytes([chunk_id.x, chunk_id.y]));
        let tick = singleton.tick.wrapping_add(tick_offset);
        let downgrade = tick.every(Ticks::from_whole_secs(60));

        for (tower_id, tower) in self.iter_mut(chunk_id) {
            // Un-owned towers must not have rulers.
            debug_assert!(tower.player_id.is_some() || !tower.units.has_ruler());

            let mut deploy = false;
            if tick.every(Ticks::from_whole_secs(if tower.player_id.is_some() {
                30
            } else {
                10
            })) {
                deploy |= tower.diminish_units_if_dead_or_overflow() != 0 && tower.active();
            }

            // Either delay or generate/decay, but not both.
            if let Some(delay) = tower.delay {
                tower.delay = NonZeroU8::new(delay.get() - 1);
            } else if tower.player_id.is_some() {
                for unit in Unit::iter() {
                    if let Some(period) = tower.tower_type.unit_generation(unit) {
                        if tick.every(period) {
                            // Add 2 but subtract up to 1 of the added ones to see if there is room.
                            let a = tower.units.add_to_tower(unit, 2, tower.tower_type, false);
                            tower.units.subtract(unit, a.saturating_sub(1));
                            deploy |= unit.is_mobile(Some(tower.tower_type)) && a < 2;
                        }
                    }
                }
            } else if let Some(downgrade) = tower.tower_type.downgrade().filter(|_| downgrade) {
                tower.tower_type = downgrade;
                tower.reconcile_units();
            }

            if deploy && !tower.units.has_ruler() {
                if let Some(path) = tower.supply_line.as_ref() {
                    // Don't send soldiers along nuke supply line.
                    if tower.force_units().max_edge_distance() >= tower.tower_type.ranged_distance()
                    {
                        for AddressedChunkEvent { dst, event } in tower.deploy_force(path.clone()) {
                            on_event(dst, event); // TODO make on_event take AddressedChunkEvent.
                        }
                    }
                }
            }

            // Force vs. force.
            if !tower.inbound_forces.is_empty() && !tower.outbound_forces.is_empty() {
                tower.inbound_forces.retain_mut(|inbound_force| {
                    if tower
                        .outbound_forces
                        .iter()
                        .all(|outbound_force| inbound_force.player_id == outbound_force.player_id)
                    {
                        // Optimization: avoid a bunch of calculations inside large territories.
                        return true;
                    }

                    let inbound_progress_required = inbound_force.progress_required() as u16;
                    let inbound_path_progress = inbound_progress_required.saturating_sub(
                        inbound_force
                            .path_progress
                            .saturating_add(inbound_force.progress_per_tick())
                            as u16,
                    );
                    let inbound_next_path_progress = inbound_progress_required
                        .saturating_sub(inbound_force.path_progress as u16);

                    //println!("inbound: {} {} {}", inbound_progress_required, inbound_path_progress, inbound_next_path_progress);

                    let mut inbound_survived = true;

                    tower.outbound_forces.retain_mut(|outbound_force| {
                        if !inbound_survived {
                            // Inbound is dead, it can no longer engage outbounds.
                            return true;
                        }

                        if outbound_force.current_destination() != inbound_force.current_source() {
                            // Not on same path.
                            return true;
                        }

                        if outbound_force.player_id == inbound_force.player_id {
                            // Optimization; this would get caught later, but avoid
                            // doing some math first.
                            return true;
                        }

                        #[allow(clippy::overly_complex_bool_expr)]
                        let debug = false && {
                            // Don't debug bot against bot.
                            !(inbound_force.player_id.map(|p| p.is_bot()).unwrap_or(true)
                                && outbound_force.player_id.map(|p| p.is_bot()).unwrap_or(true))
                        };

                        let outbound_progress_required = outbound_force.progress_required() as u16;
                        let effective_inbound_path_progress =
                            inbound_path_progress * outbound_progress_required;
                        let effective_inbound_next_path_progress =
                            inbound_next_path_progress * outbound_progress_required;

                        let effective_outbound_path_progress =
                            outbound_force.path_progress as u16 * inbound_progress_required;
                        let effective_outbound_next_path_progress = outbound_force
                            .path_progress
                            .saturating_add(outbound_force.progress_per_tick())
                            as u16
                            * inbound_progress_required;

                        let overlap = effective_inbound_path_progress
                            .max(effective_outbound_path_progress)
                            <= effective_inbound_next_path_progress
                                .min(effective_outbound_next_path_progress);

                        if debug && overlap {
                            println!(
                                "overlap detected {:?} {:?}",
                                inbound_force.player_id, outbound_force.player_id
                            );
                        }

                        if overlap
                            && relationship(inbound_force.player_id, outbound_force.player_id)
                                .is_unfriendly(false)
                        {
                            let position = inbound_force.interpolated_position(0.0);

                            // Only one version of the battle will generate events (guaranteeing
                            // consistency even if the fighting isn't commutative).
                            let authoritative_events = inbound_force.current_destination()
                                > outbound_force.current_destination();

                            let winner = Combatants::fight(
                                &mut Combatants::force(&mut inbound_force.units),
                                &mut Combatants::force(&mut outbound_force.units),
                                |info| {
                                    if authoritative_events {
                                        context(info.into_info_event(
                                            position,
                                            inbound_force.player_id,
                                            outbound_force.player_id,
                                        ));
                                    }
                                },
                            );

                            if debug {
                                println!("overlap at {:?} -> w={winner:?}", position,);
                            }

                            inbound_survived &= winner == Some(CombatSide::Attacker);
                            winner == Some(CombatSide::Defender)
                        } else {
                            true
                        }
                    });

                    inbound_survived
                });
            }

            let position = tower_id.as_vec2();

            // Force vs. tower.
            for mut force in tower.inbound_forces.extract_if(|f| f.tick(tower_id)) {
                let tower_player_id = tower.player_id;
                if tower_player_id.is_some() || !tower.units.is_empty() {
                    let force_player_id = force.player_id;
                    if relationship(tower_player_id, force_player_id)
                        .is_unfriendly(force.units.has_ruler())
                    {
                        let mut force_combatants = Combatants::force(&mut force.units);
                        let mut tower_combatants =
                            Combatants::tower(tower.tower_type, &mut tower.units);

                        let mut tower_emped = false;
                        let winner = Combatants::fight(
                            &mut force_combatants,
                            &mut tower_combatants,
                            |info| {
                                tower_emped |= info == CombatInfo::Emp(CombatSide::Attacker);
                                context(info.into_info_event(
                                    position,
                                    force_player_id,
                                    tower_player_id,
                                ));
                            },
                        );

                        if tower_emped {
                            let emp_delay = NonZeroU8::new(
                                Ticks::from_whole_secs(Unit::EMP_SECONDS as TicksRepr)
                                    .0
                                    .try_into()
                                    .unwrap(),
                            )
                            .unwrap();
                            tower.delay = tower.delay.max(Some(emp_delay));
                        }

                        if winner != Some(CombatSide::Attacker) {
                            if let Some(force_player_id) = force_player_id {
                                context(InfoEvent {
                                    position,
                                    info: Info::LostForce(force_player_id),
                                });
                            }
                        }

                        if winner != Some(CombatSide::Defender) {
                            if let Some(tower_player_id) = tower_player_id {
                                context(InfoEvent {
                                    position,
                                    info: Info::LostTower {
                                        tower_id,
                                        player_id: tower_player_id,
                                        reason: if winner == Some(CombatSide::Attacker) {
                                            LostTowerReason::CapturedBy(force_player_id)
                                        } else {
                                            LostTowerReason::DestroyedBy(force_player_id)
                                        },
                                    },
                                });
                            }

                            let new_player_id = if let Some(force_player_id) =
                                force_player_id.filter(|_| winner == Some(CombatSide::Attacker))
                            {
                                context(InfoEvent {
                                    position,
                                    info: Info::GainedTower {
                                        tower_id,
                                        player_id: force_player_id,
                                        reason: GainedTowerReason::CapturedFrom(tower_player_id),
                                    },
                                });
                                Some(force_player_id)
                            } else {
                                None
                            };

                            // Don't crash when zombies are nuked.
                            if tower.player_id.is_some() || new_player_id.is_some() {
                                Tower::set_player_id_inner(
                                    &mut tower.player_id,
                                    &tower.units,
                                    &mut tower.supply_line,
                                    new_player_id,
                                );
                            }

                            // Not captured, blown up.
                            if tower.player_id.is_none() {
                                while let Some(downgrade) = tower.tower_type.downgrade() {
                                    tower.tower_type = downgrade;
                                }
                                tower.delay = None;
                            }
                        }
                    }

                    // else: force may  move on (or start its journey).
                } else if let Some(force_player_id) =
                    force.player_id.filter(|_| force.units.is_alive())
                {
                    // Force explored a tower.
                    context(InfoEvent {
                        position: tower_id.as_vec2(),
                        info: Info::GainedTower {
                            tower_id,
                            player_id: force_player_id,
                            reason: GainedTowerReason::Explored,
                        },
                    });

                    // Cannot borrow so manually inline functions.
                    Tower::set_player_id_inner(
                        &mut tower.player_id,
                        &tower.units,
                        &mut tower.supply_line,
                        Some(force_player_id),
                    );
                    tower
                        .units
                        .reconcile(tower.tower_type, tower.player_id.is_some());
                }

                let relationship = relationship(force.player_id, tower.player_id);

                if force.units.is_empty() {
                    // Drop.
                } else if force.fuel == 0 {
                    // Expire.
                    if let Some(player_id) = force.player_id {
                        context(InfoEvent {
                            position: tower_id.as_vec2(),
                            info: Info::LostForce(player_id),
                        });
                    }
                } else if matches!(relationship, Relationship::Ally | Relationship::Comrade)
                    && force.try_move_on(
                        tower.tower_type,
                        &mut tower.units,
                        tower.player_id.filter(|_| relationship.is_ally()),
                        tower.supply_line.as_ref(),
                    )
                {
                    if force.units.is_many()
                        && tower
                            .outbound_forces
                            .iter()
                            .filter(|f| f.current_destination() == force.current_destination())
                            .count() as u32
                            >= 8
                    {
                        // Cramming.
                        if let Some(player_id) = force.player_id {
                            context(InfoEvent {
                                position: tower_id.as_vec2(),
                                info: Info::LostForce(player_id),
                            });
                        }
                    } else {
                        let (chunk_id, tower_id) = force.current_source().split();
                        on_event(chunk_id, ChunkEvent::add_outbound_force(tower_id, &force));

                        let (chunk_id, tower_id) = force.current_destination().split();
                        on_event(chunk_id, ChunkEvent::AddInboundForce { tower_id, force });
                    }
                } else if relationship.is_friendly(force.units.has_ruler()) {
                    // Force arrived.
                    // Only real players are eligible for overflowing forces.
                    tower.units.add_units_to_tower(
                        force.units,
                        tower.tower_type,
                        tower.player_id.is_some(),
                    );
                } else if force.units.available(Unit::Ruler) > 0 {
                    if let Some(player_id) = force.player_id {
                        context(InfoEvent {
                            position,
                            info: Info::LostRuler {
                                player_id,
                                reason: LostRulerReason::KilledBy(None, Unit::Shield),
                            },
                        })
                    } else {
                        debug_assert!(false, "force with ruler didn't have player");
                    }
                }
            }

            tower
                .outbound_forces
                .retain_mut(|force| !force.raw_tick(None));

            shrink_vec(&mut tower.inbound_forces);
            shrink_vec(&mut tower.outbound_forces);
        }
    }
}

// TODO does this belong here?
enum Relationship {
    /// Same player or both zombies.
    Comrade,
    /// Allied player.
    Ally,
    /// Enemy player/zombie.
    Enemy,
}

impl Relationship {
    #[allow(unused)]
    fn is_comrade(&self) -> bool {
        matches!(self, Self::Comrade)
    }

    fn is_ally(&self) -> bool {
        matches!(self, Self::Ally)
    }

    fn is_friendly(&self, ruler_arriving_at_tower: bool) -> bool {
        match self {
            Self::Comrade => true,
            Self::Ally => !ruler_arriving_at_tower,
            Self::Enemy => false,
        }
    }

    fn is_unfriendly(&self, ruler_arriving_at_tower: bool) -> bool {
        !self.is_friendly(ruler_arriving_at_tower)
    }
}
