// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chunk::*;
use crate::info::*;
use crate::player::*;
use crate::singleton::*;
use crate::tower::{integer_sqrt, TowerId};
use kodiak_common::actor_model::*;
use kodiak_common::bitcode::{self, *};
use kodiak_common::{
    apply, apply_inputs, define_actor_state, define_events, define_world, singleton, singleton_mut,
    PlayerId,
};
use std::collections::BTreeMap;

mod towers;
pub use towers::{ChunkMap, WorldChunks};

// TODO find better spot for this.
impl ActorId for ChunkId {
    type DenseMap<T> = ChunkMap<T>;
    type SparseMap<T> = BTreeMap<Self, T>; // TODO SparseChunkMap
    type Map<T> = SortedVecMap<Self, T>;
}

impl Actor for Chunk {
    type Id = ChunkId;
    const KEEPALIVE: u8 = 16;
}

define_events!(Chunk, Server, ChunkMaintenance, ChunkInput; Encode, Decode);
define_events!(Chunk, ChunkId, ChunkHaltEvent, ChunkEvent; Encode, Decode);
define_actor_state!(Chunk, Server, ChunkId; Encode, Decode);
define_events!(Player, Server, PlayerMaintainance, PlayerInput; Encode, Decode);
define_actor_state!(Player, Server; Encode, Decode);
define_events!(Singleton, Server, SingletonInput; Encode, Decode);
define_actor_state!(Singleton, Server; Encode, Decode);
define_world!((), Chunk, Player, Singleton; Encode, Decode); // todo cksum

impl WorldTick<OnInfo<'_>> for World {
    fn tick_before_inputs(&mut self, context: &mut OnInfo<'_>) {
        let Some(singleton) = singleton_mut!(self) else {
            return;
        };
        singleton.tick = singleton.tick.next();

        // TODO move to halt.rs
        let mut halt_events = vec![];
        for (upstream_chunk_id, state) in Map::iter(&self.chunk) {
            let upstream_chunk: &Chunk = &state.actor;

            for (upstream_tower_id, upstream_tower) in upstream_chunk.iter(upstream_chunk_id) {
                for (i, force) in upstream_tower.inbound_forces.iter().enumerate() {
                    let Some(upstream_player_id) = force.player_id else {
                        debug_assert!(false, "todo zombies?");
                        continue;
                    };
                    let upstream_player = Self::player_inner(&self.player, upstream_player_id);

                    let remaining_path = force.path().iter().skip(2);
                    for downstream_chunk_id in self.halt_path(remaining_path, upstream_player) {
                        halt_events.push((
                            upstream_chunk_id,
                            (
                                downstream_chunk_id,
                                ChunkHaltEvent::Force(upstream_tower_id.into(), i as u32),
                            ),
                        ));
                    }
                }

                // Check supply line.
                let Some(supply_line) = &upstream_tower.supply_line else {
                    continue;
                };
                let Some(player_id) = upstream_tower.player_id else {
                    debug_assert!(false, "supply line without player");
                    continue;
                };
                let upstream_player = Self::player_inner(&self.player, player_id);

                for downstream_chunk_id in self.halt_path(supply_line.iter(), upstream_player) {
                    halt_events.push((
                        upstream_chunk_id,
                        (
                            downstream_chunk_id,
                            ChunkHaltEvent::SupplyLine(upstream_tower_id.into()),
                        ),
                    ));
                }
            }
        }

        self.extend(halt_events);
        apply!(self, Chunk, ChunkId, ChunkHaltEvent, context);

        for (_, state) in Map::iter_mut(&mut self.player) {
            state.actor.new_alliances.clear();
        }

        let singleton = singleton!(self).unwrap();
        let mut chunk_events = vec![];
        for (chunk_id, state) in Map::iter_mut(&mut self.chunk) {
            let chunk: &mut Chunk = &mut state.actor;
            chunk.tick(
                chunk_id,
                |player_id| Self::player_inner(&self.player, player_id),
                singleton,
                |dst, e| chunk_events.push((dst, (chunk_id, e))),
                context,
            )
        }
        self.extend(chunk_events);
    }

    fn tick_after_inputs(&mut self, context: &mut OnInfo<'_>) {
        // We apply chunk events after inputs since `ChunkInput`s may create `ChunkEvent`s.
        // TODO detect if events weren't applied in debug mode.
        apply!(self, Chunk, ChunkId, ChunkEvent, context);
    }

    fn tick_client(&mut self, context: &mut OnInfo<'_>) {
        apply_inputs!(self, Chunk, ChunkMaintenance, context);
        apply_inputs!(self, Player, PlayerMaintainance, context);
        self.tick_before_inputs(context);
        {
            let mut context = OnChunkEvent::new(&mut *context);
            apply_inputs!(self, Chunk, ChunkInput, &mut context);
            self.extend(context.into_events());
        }
        apply_inputs!(self, Player, PlayerInput, context);
        apply_inputs!(self, Singleton, SingletonInput, context);
        self.tick_after_inputs(context);
    }
}

impl World {
    pub const MAX_ROAD_LENGTH: u32 = 5;
    pub const MAX_ROAD_LENGTH_SQUARED: u64 = (Self::MAX_ROAD_LENGTH as u64 + 1).pow(2) - 1;
    pub const MAX_PATH_ROADS: usize = 16;

    pub const CENTER: TowerId =
        TowerId::new(WorldChunks::SIZE as u16 / 2, WorldChunks::SIZE as u16 / 2);

    /// Returns an iterator of chunks that send halt events to `path`.
    fn halt_path<'a>(
        &'a self,
        path: impl Iterator<Item = TowerId> + 'a,
        player: &'a Player,
    ) -> impl Iterator<Item = ChunkId> + 'a {
        let no_new_alliances = player.new_alliances.is_empty();
        let mut dedup = vec![];

        // TODO optimization (not supply line):
        // TODO only first item in path for tower destroyed
        // TODO only first item in path for new alliance (using time since alliance was created and force age).

        path.filter_map(move |tower_id| {
            let (chunk_id, tower_id) = tower_id.split();
            let Some(chunk) = Map::get(&self.chunk, chunk_id) else {
                // Chunk not visible.
                return None;
            };

            if let Some(tower) = chunk.actor.get(tower_id) {
                if no_new_alliances {
                    return None; // Optimization (avoid looking up player).
                }
                let Some(tower_player) = tower.player_id else {
                    return None;
                };
                if !player.new_alliances.contains(&tower_player) {
                    return None;
                }
                // Halt because of alliance.
            } else {
                // Halt because tower was destroyed.
            }

            // We only need 1 halt event per chunk.
            if dedup.contains(&chunk_id) {
                return None;
            }
            dedup.push(chunk_id);
            Some(chunk_id)
        })
    }

    pub fn singleton(&self) -> &Singleton {
        singleton!(self).expect("no singleton")
    }

    pub fn have_alliance(&self, a: PlayerId, b: PlayerId) -> bool {
        Self::have_alliance_inner(&self.player, a, b)
    }

    fn have_alliance_inner(
        players: &<PlayerId as ActorId>::DenseMap<PlayerState>,
        a: PlayerId,
        b: PlayerId,
    ) -> bool {
        Self::player_inner(players, a).allies.contains(&b)
            && Self::player_inner(players, b).allies.contains(&a)
    }

    pub fn player(&self, player_id: PlayerId) -> &Player {
        Self::player_inner(&self.player, player_id)
    }

    fn player_inner(player: &impl Map<PlayerId, PlayerState>, player_id: PlayerId) -> &Player {
        &Map::get(player, player_id)
            .unwrap_or_else(|| {
                panic!(
                    "missing player {player_id:?}, is_bot: {}",
                    player_id.is_bot()
                );
            })
            .actor
    }

    #[cfg(feature = "server")]
    pub fn new() -> Self {
        Self {
            chunk: ChunkMap::from_fn(|id| Some(Chunk::new(id).into())),
            player: Default::default(),
            singleton: Some((
                SingletonId,
                Singleton {
                    tick: Default::default(),
                }
                .into(),
            )),
        }
    }

    #[cfg(feature = "server")]
    pub fn dispatch_chunk_maintenance(
        &mut self,
        chunk_id: ChunkId,
        input: ChunkMaintenance,
        on_info: &mut OnInfo,
    ) {
        let on_info = &mut kodiak_common::actor_model::Dst::new(on_info, chunk_id);
        Map::get_mut(&mut self.chunk, chunk_id)
            .unwrap()
            .apply_owned(input, on_info);
    }

    #[cfg(feature = "server")]
    pub fn dispatch_chunk_input(
        &mut self,
        chunk_id: ChunkId,
        input: ChunkInput,
        on_info: &mut OnInfo,
    ) {
        let mut context = OnChunkEvent::new(on_info);

        Map::get_mut(&mut self.chunk, chunk_id)
            .unwrap()
            .apply_owned(input, &mut context);
        self.extend(context.into_events());
    }

    #[cfg(feature = "server")]
    pub fn dispatch_player_maintenance(
        &mut self,
        player_id: PlayerId,
        input: PlayerMaintainance,
        mut on_info: impl FnMut(InfoEvent),
    ) {
        Map::get_mut(&mut self.player, player_id)
            .unwrap()
            .apply_owned(input, &mut on_info);
    }

    #[cfg(feature = "server")]
    pub fn dispatch_player_input(
        &mut self,
        player_id: PlayerId,
        input: PlayerInput,
        mut on_info: impl FnMut(InfoEvent),
    ) {
        Map::get_mut(&mut self.player, player_id)
            .unwrap()
            .apply_owned(input, &mut on_info)
    }

    #[cfg(feature = "server")]
    pub fn dispatch_singleton_input(
        &mut self,
        input: SingletonInput,
        mut on_info: impl FnMut(InfoEvent),
    ) {
        Map::get_mut(&mut self.singleton, SingletonId)
            .unwrap()
            .apply_owned(input, &mut on_info)
    }

    pub fn find_best_path(
        &self,
        src: TowerId,
        dst: TowerId,
        max_edge_distance: Option<u32>,
        player_id: PlayerId,
        filter: impl Fn(TowerId) -> bool,
    ) -> Option<Vec<TowerId>> {
        if let Some(d) = max_edge_distance {
            (src.distance(dst) <= d && filter(dst)).then(|| vec![src, dst])
        } else {
            self.astar(src, dst, player_id, &filter)
                .ok()
                .filter(|p| p.len() >= 2)
        }
    }

    pub fn find_best_incomplete_path(
        &self,
        src: TowerId,
        dst: TowerId,
        max_edge_distance: Option<u32>,
        player_id: PlayerId,
        filter: impl Fn(TowerId) -> bool,
    ) -> Vec<TowerId> {
        if let Some(d) = max_edge_distance {
            (src.distance(dst) <= d && filter(dst))
                .then(|| vec![src, dst])
                .unwrap_or_else(|| vec![src])
        } else {
            self.astar(src, dst, player_id, &filter)
                .unwrap_or_else(|reachable| {
                    self.astar(src, reachable, player_id, &filter)
                        .unwrap_or_default()
                })
        }
    }

    fn astar(
        &self,
        src: TowerId,
        dst: TowerId,
        player_id: PlayerId,
        filter: &impl Fn(TowerId) -> bool,
    ) -> Result<Vec<TowerId>, TowerId> {
        // Scale distances squared up to avoid integer rounding errors (basically a fixed point).
        // Only 33 bits of u64 distance_squared are ever used so a D2_SCALE <= 2^30 is valid.
        const D2_SCALE: u64 = 1 << 16;
        const D_SCALE: u32 = 1 << 8; // Must be square root of D2_SCALE;

        let dst_player_id = self.chunk.get(dst).and_then(|t| t.player_id);
        let mut shortest = (src, u32::MAX);

        // Don't visit every tower if a path isn't found in a reasonable time.
        let mut emergency_stop: u16 = 128 + src.distance(dst).max(2048) as u16;

        pathfinding::directed::astar::astar(
            &src,
            |&pos| {
                pos.neighbors().filter_map(move |tower_id| {
                    self.chunk.get(tower_id).and_then(|t| {
                        let passes_through_allicance = t.player_id.is_some_and(|p| {
                            Some(p) != dst_player_id && self.have_alliance(player_id, p)
                        });
                        (!passes_through_allicance && filter(tower_id)).then(|| {
                            let d2 = pos.distance_squared(tower_id);
                            (tower_id, integer_sqrt(d2 * D2_SCALE))
                        })
                    })
                })
            },
            |&pos| {
                let mut heuristic = integer_sqrt(pos.distance_squared(dst) * D2_SCALE);
                let tower = self.chunk.get(pos).unwrap();
                if tower.player_id == Some(player_id) {
                    heuristic += 2 * D_SCALE; // Prioritize claiming new towers.
                } else if tower.player_id.is_some() || !tower.units.is_empty() {
                    heuristic += 32 * D_SCALE; // Deprioritize going through enemy/zombies.
                }
                if heuristic < shortest.1 {
                    shortest = (pos, heuristic);
                }
                heuristic
            },
            |pos| {
                emergency_stop -= 1;
                pos == &dst || emergency_stop == 0
            },
        )
        .filter(|_| emergency_stop > 0)
        .map(|(path, _)| path)
        .ok_or(shortest.0)
    }

    #[inline]
    pub fn distance_squared_to_center(tower_id: TowerId) -> u64 {
        Self::CENTER.distance_squared(tower_id)
    }
}

/*
/// Context needed during ChunkInput apply.
struct InputContext<I> {
    events: Vec<(ChunkId, (ChunkId, ChunkEvent))>,
    on_info: I,
}

impl<I: OnInfo> OnInfo for InputContext<&mut I> {
    fn on_info(&mut self, info: InfoEvent) {
        self.on_info.on_info(info);
    }
}

impl<I> OnChunkEvent for InputContext<I> {
    fn on_chunk_event(&mut self, src: ChunkId, event: AddressedChunkEvent) {
        self.events.push((event.dst, (src, event.event)))
    }
}
*/

#[cfg(test)]
mod tests {
    use crate::tower::integer_sqrt;
    use crate::world::World;

    #[test]
    fn max_edge_distance() {
        for i in 0..=10000 {
            debug_assert_eq!(
                integer_sqrt(i) as u32 <= World::MAX_ROAD_LENGTH,
                i <= World::MAX_ROAD_LENGTH_SQUARED,
                "{}",
                i
            )
        }
    }
}
