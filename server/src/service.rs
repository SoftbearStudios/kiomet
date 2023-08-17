// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bot::TowerBot;
use crate::regulator::Regulator;
use atomic_refcell::AtomicRef;
use common::alerts::{AlertFlag, Alerts};
use common::chunk::{ChunkId, ChunkRectangle};
use common::death_reason::DeathReason;
use common::info::{GainedTowerReason, Info, InfoEvent, LostRulerReason};
use common::player::Player;
use common::protocol::{Command, Diff, NonActor, Update};
use common::singleton::SingletonId;
use common::ticks::Ticks;
use common::tower::{TowerArray, TowerId, TowerRectangle};
use common::unit::Unit;
use common::world::{Knowledge, Visibility, World, WorldChunks};
use common_util::actor2::WorldTick;
use common_util::storage::Map;
use core_protocol::id::{GameId, PlayerId};
use fxhash::FxHashSet;
use game_server::context::Context;
use game_server::game_service::GameArenaService;
use game_server::player::{PlayerRepo, PlayerTuple};
use log::warn;
use std::cmp::Ordering;
use std::sync::Arc;
use std::time::Duration;

pub struct TowerService {
    maybe_dead: FxHashSet<PlayerId>,
    pub regulator: Regulator,
    pub world: World,
}

#[derive(Debug, Default)]
pub struct ClientData {
    knowledge: Knowledge,
    non_actor: NonActor,
    viewport: ChunkRectangle,
}

#[derive(Clone, Debug, Default)]
pub struct PlayerData {
    pub alive: bool,
    pub towers: FxHashSet<TowerId>,
    /// Saturating counter of how long the player has lived.
    pub lifetime: Ticks,
    /// Clamped to 255.
    pub tower_counts: TowerArray<u8>,
    /// If dead, this is the reason why.
    pub death_reason: Option<DeathReason>,
    /// Cached alerts (some of which are used as persistent storage).
    pub(crate) alerts: Alerts,
}

impl GameArenaService for TowerService {
    const GAME_ID: GameId = GameId::Kiomet;
    const TICK_PERIOD_SECS: f32 = Ticks::PERIOD_SECS;
    const LIMBO: Duration = Duration::from_secs(12);
    #[cfg(debug_assertions)]
    const LEADERBOARD_MIN_PLAYERS: usize = 0;
    #[cfg(not(debug_assertions))]
    const LEADERBOARD_MIN_PLAYERS: usize = 5;
    #[cfg(debug_assertions)]
    const LIVEBOARD_BOTS: bool = true;
    type Bot = TowerBot;
    type ClientData = ClientData;
    type GameUpdate = Update;
    type GameRequest = Command;
    type PlayerData = PlayerData;
    type PlayerExtension = ();

    fn new(_: usize) -> Self {
        print!("Generating world...");
        let world = World::new(); // TODO Default?
        println!("done!");

        Self {
            maybe_dead: Default::default(),
            regulator: Default::default(),
            world,
        }
    }

    fn player_joined(
        &mut self,
        player_tuple: &Arc<PlayerTuple<Self>>,
        _players: &PlayerRepo<Self>,
    ) {
        let player_id = player_tuple.borrow_player().player_id;
        if self.regulator.join(player_id) {
            self.world
                .player
                .insert(player_id, Player::default().into());
        }
    }

    fn player_command(
        &mut self,
        command: Self::GameRequest,
        player_tuple: &Arc<PlayerTuple<Self>>,
        players: &PlayerRepo<Self>,
    ) -> Option<Self::GameUpdate> {
        let player_id = player_tuple.borrow_player().player_id;
        if !self.regulator.active(player_id) {
            return None;
        }

        fn wrap(path: &str) -> impl Fn(&str) -> String + '_ {
            move |e| format!("{path} resulted in {e}")
        }

        if let Err(e) = (|| match command {
            Command::Alliance {
                with,
                break_alliance,
            } => self
                .alliance(player_id, with, break_alliance, players)
                .map_err(wrap("Alliance")),
            Command::DeployForce { tower_id, path } => self
                .deploy_force(player_id, tower_id, path, players)
                .map_err(wrap("DeployForce")),
            Command::SetSupplyLine { tower_id, path } => {
                if let Some(path) = path
                    .as_ref()
                    .filter(|_| {
                        self.world.chunk.get(tower_id).map_or(false, |t| {
                            let mut mobile = false;
                            let max_edge_distance = t.tower_type.ranged_distance();

                            for (u, _) in t.units.iter() {
                                if !u.is_mobile(Some(t.tower_type)) {
                                    continue;
                                }
                                mobile = true;

                                // Don't attempt to send soldiers/etc. on nuke supply line.
                                if u.ranged_distance() != max_edge_distance {
                                    return false;
                                }
                            }
                            mobile
                        })
                    })
                    .cloned()
                {
                    self.deploy_force(player_id, tower_id, path, players)
                        .map_err(wrap("SetSupplyLine/DeployForce"))?;
                }
                self.set_supply_line(player_id, tower_id, path, players)
                    .map_err(wrap("SetSupplyLine"))
            }
            Command::SetViewport(viewport) => {
                let mut player = player_tuple.borrow_player_mut();
                if let Some(client) = player.client_mut() {
                    client.data_mut().viewport = viewport;
                    Ok(())
                } else {
                    debug_assert!(false);
                    Err("bots can't set viewport")
                }
                .map_err(wrap("SetViewport"))
            }
            Command::Spawn => self.spawn_player(player_id, players).map_err(wrap("Spawn")),
            Command::Upgrade {
                tower_id,
                tower_type,
            } => self
                .upgrade_tower(player_id, tower_id, tower_type, players)
                .map_err(wrap("Upgrade")),
        })() {
            if !player_tuple.borrow_player().is_bot() {
                warn!("{}", e);
            }
        }
        None
    }

    fn player_left(&mut self, player_tuple: &Arc<PlayerTuple<Self>>, _: &PlayerRepo<Self>) {
        let player_id = player_tuple.borrow_player().player_id;
        self.regulator.leave(player_id);

        // Can't kill since we are in the ChunkInput phase and kill is ChunkMaintenance.
        self.maybe_dead.insert(player_id);
    }

    fn get_game_update(
        &self,
        player_tuple: &Arc<PlayerTuple<Self>>,
        client_data: &mut Self::ClientData,
        _players: &PlayerRepo<Self>,
    ) -> Option<Self::GameUpdate> {
        let player = player_tuple.borrow_player();
        if !self.regulator.active(player.player_id) {
            return None;
        }
        let admin = if let Some(client) = player.client() {
            client.admin || cfg!(debug_assertions) /* || true */
        } else {
            debug_assert!(false);
            false
        };

        let player_id = player.player_id;
        let player = AtomicRef::map(player, |player| &player.data);

        let bounding_rectangle = if player.towers.is_empty() {
            let middle: ChunkId = World::CENTER.into();
            ChunkRectangle {
                bottom_left: middle,
                top_right: middle,
            }
            .into()
        } else {
            let margin = player
                .tower_counts
                .iter()
                .filter(|(_, c)| **c > 0)
                .map(|(t, _)| t.sensor_radius() / TowerId::CONVERSION)
                .max()
                .unwrap_or(0)
                .clamp(3, 12);
            TowerRectangle::bounding(player.towers.iter().copied()).add_margin(margin)
        };

        debug_assert!(bounding_rectangle.is_valid());

        let effective_viewport = if admin {
            client_data.viewport
        } else {
            // Viewport clamped to bounds.
            client_data.viewport.clamp_to(bounding_rectangle.into())
        }
        .clamp_to(ChunkRectangle::new(
            ChunkId::new(0, 0),
            ChunkId::new(
                WorldChunks::SIZE_CHUNKS as u8 - 1,
                WorldChunks::SIZE_CHUNKS as u8 - 1,
            ),
        ));

        let actor_update = self.world.get_update(
            &mut client_data.knowledge,
            Visibility {
                chunk: |k: &Knowledge| {
                    let chunk_ids: FxHashSet<_> = Map::keys(&k.chunk).collect();
                    let mut governor: u8 = 6;
                    effective_viewport.into_iter().filter(move |chunk_id| {
                        chunk_ids.contains(chunk_id) || {
                            if let Some(new) = governor.checked_sub(1) {
                                governor = new;
                                true
                            } else {
                                false
                            }
                        }
                    })
                },
                player: |k: &Knowledge| {
                    // TODO remove collect (allow iterator to borrow part of knowledge).
                    let chunk_ids: Vec<_> = Map::keys(&k.chunk).collect();
                    chunk_ids
                        .into_iter()
                        .flat_map(move |chunk_id| {
                            self.world
                                .chunk
                                .get_chunk(chunk_id)
                                .unwrap()
                                .iter_player_ids()
                        })
                        .chain(Some(player_id)) // You always need your own player to know who's an ally.
                },
                singleton: |_: &_| Some(SingletonId),
            },
        );

        let non_actor = NonActor {
            alive: player.alive,
            tower_counts: player.tower_counts,
            death_reason: player.death_reason.into(),
            alerts: player.alerts,
            bounding_rectangle,
        };
        let non_actor_diff = client_data.non_actor.diff(&non_actor);
        client_data.non_actor = non_actor;

        // Always send even if there are no events, for accurate time-keeping.
        Some(Update {
            actor_update,
            non_actor_diff,
        })
    }

    fn is_alive(&self, player_tuple: &Arc<PlayerTuple<Self>>) -> bool {
        player_tuple.borrow_player().data.alive
    }

    fn tick(&mut self, context: &mut Context<Self>) {
        for mut player_ref in context.players.iter_borrow_mut() {
            let player = &mut *player_ref;
            if player.data.alive {
                player.lifetime = player.lifetime.saturating_add(Ticks::ONE);

                #[cfg(debug_assertions)]
                if self.counter().every(Ticks::from_whole_secs(20))
                    && matches!(player.alias().as_str(), "chonk" | "squonk")
                {
                    use common::chunk::ChunkInput;
                    use common::force::{Force, Path};

                    let alias = player.alias();
                    let radius = if &*alias == "chonk" {
                        500 // Chonk is a giant circle about the size of Debased.
                    } else {
                        10000 // Squonk is the whole world, aka Square + Chonk.
                    };
                    println!("{}", alias);

                    let mut events = Vec::new();
                    for (tower_id, tower) in
                        self.world.chunk.iter_towers_circle(World::CENTER, radius)
                    {
                        if tower.player_id == Some(player.player_id) {
                            continue;
                        }
                        let mut units = common::units::Units::default();
                        units.add(Unit::Bomber, 5);

                        let mut src = World::CENTER;
                        if tower_id == src {
                            src = src.connectivity_id().unwrap(); // We can't send from a tower to itself.
                        }

                        let force =
                            Force::new(player.player_id, units, Path::new(vec![src, tower_id]));
                        let (chunk_id, tower_id) = tower_id.split();
                        events.push((chunk_id, ChunkInput::AddInboundForce { tower_id, force }));
                    }
                    for (chunk_id, event) in events {
                        self.world
                            .dispatch_chunk_input(chunk_id, event, |_| unreachable!());
                    }
                }

                if self.counter().every(Ticks::from_whole_secs(1)) {
                    player.score = 0;
                    let mut tower_counts: TowerArray<u8> = TowerArray::default();

                    let alerts = &mut player.data.alerts;
                    alerts.reset_ephemeral();
                    let mut flags = alerts.flags();

                    // Assume ruler is not safe until proven otherwise.
                    flags |= AlertFlag::RulerNotSafe;
                    for &tower_id in &player.data.towers {
                        if let Some(tower) = self.world.chunk.get(tower_id) {
                            if tower.units.has_ruler() {
                                alerts.ruler_position = Some(tower_id);
                                if (tower.units.available(Unit::Shield)
                                    >= Unit::damage_to_finite(tower.tower_type.max_ranged_damage())
                                        as usize
                                    || tower_id.neighbors().all(|neighbor_id| {
                                        self.world
                                            .chunk
                                            .get(neighbor_id)
                                            .map_or(true, |t| t.player_id == Some(player.player_id))
                                    }))
                                    && tower
                                        .inbound_forces
                                        .iter()
                                        .all(|f| f.player_id == Some(player.player_id))
                                {
                                    flags -= AlertFlag::RulerNotSafe;
                                } else if tower
                                    .inbound_forces
                                    .iter()
                                    .any(|f| f.player_id != Some(player.player_id))
                                {
                                    flags |= AlertFlag::RulerUnderAttack;
                                }
                            } else if tower.active() {
                                for unit in Unit::iter() {
                                    if !unit.is_mobile(Some(tower.tower_type))
                                        || !tower.units.contains(unit)
                                    {
                                        continue;
                                    }
                                    let generates = tower.unit_generation(unit).is_some();
                                    if generates && tower.supply_line.is_some() {
                                        // Problem will go away.
                                        continue;
                                    }
                                    // TODO: Find the *worst* offending towers..
                                    match tower
                                        .units
                                        .available(unit)
                                        .cmp(&tower.units.capacity(unit, Some(tower.tower_type)))
                                    {
                                        Ordering::Greater => alerts.overflowing = Some(tower_id),
                                        Ordering::Equal if generates => {
                                            alerts.full = Some(tower_id)
                                        }
                                        _ => {}
                                    }
                                }
                            }

                            if tower.inbound_forces.iter().any(|f| f.player_id.is_none()) {
                                alerts.zombies = Some(tower_id);
                            }

                            // Don't count inactive towers towards tower counts.
                            if !tower.active() {
                                continue;
                            }

                            player.score =
                                player.score.saturating_add(tower.tower_type.score_weight());

                            tower_counts[tower.tower_type] =
                                tower_counts[tower.tower_type].saturating_add(1);
                        } else {
                            debug_assert!(false, "missing tower");
                        };
                    }

                    alerts.set_flags(flags);
                    player.tower_counts = tower_counts;
                }
            }
        }

        self.world
            .tick_after_inputs(&mut Self::on_info_event(&context.players, |_| {
                unreachable!("tick_after_inputs killed player")
            }));

        self.regulator.tick(|player_id, joining| {
            if joining {
                self.world
                    .player
                    .insert(player_id, Player::default().into());
            } else {
                self.world.player.remove(&player_id);
            }
        });
    }

    fn post_update(&mut self, context: &mut Context<Self>) {
        self.world.post_update();

        // Boundary between old tick and new tick.

        // Take to avoid borrowing issue.
        let mut maybe_dead = std::mem::take(&mut self.maybe_dead);
        for player_id in maybe_dead.drain() {
            // Makes `ChunkMaintenance`s which have to run before tick_before_inputs.
            self.kill_player(player_id, &context.players);
        }
        self.maybe_dead = maybe_dead;

        if self.counter().next().every(Ticks::from_whole_secs(8)) {
            // Makes `ChunkMaintenance`s which have to run before tick_before_inputs.
            self.shrink(&context.players);
        }

        self.world
            .tick_before_inputs(&mut Self::on_info_event(&context.players, |player_id| {
                self.maybe_dead.insert(player_id);
            }));

        /*
        for player_id in context.players.iter_player_ids() {
            if thread_rng().gen_bool(0.25) {
                self.maybe_dead.insert(player_id);
            }
        }
        */
    }

    fn world_size(&self) -> f32 {
        self.world
            .chunk
            .iter()
            .map(|(chunk_id, chunk)| chunk.actor.iter(chunk_id).count())
            .sum::<usize>() as f32
    }

    fn entities(&self) -> usize {
        self.world
            .chunk
            .iter()
            .map(|(chunk_id, chunk)| {
                chunk
                    .actor
                    .iter(chunk_id)
                    .map(|(_, tower)| tower.inbound_forces.len() + tower.outbound_forces.len())
                    .sum::<usize>()
            })
            .sum::<usize>()
            / 2
    }
}

impl TowerService {
    fn counter(&self) -> Ticks {
        self.world.singleton().tick
    }

    pub(crate) fn on_info_event<'a>(
        players: &'a PlayerRepo<Self>,
        mut maybe_dead: impl FnMut(PlayerId) + 'a,
    ) -> impl FnMut(InfoEvent) + 'a {
        move |info_event| match info_event.info {
            Info::GainedTower {
                tower_id,
                player_id,
                reason,
            } => {
                if let Some(mut new_player) = players.borrow_player_mut(player_id) {
                    if let GainedTowerReason::Spawned = reason {
                        debug_assert!(!new_player.alive, "spawning player should not be alive");
                        new_player.alive = true;
                    }

                    let inserted = new_player.towers.insert(tower_id);
                    debug_assert!(
                        inserted,
                        "tower {:?} was already in set of {:?} but now inserted due to {:?}",
                        tower_id, player_id, reason
                    );
                } else {
                    debug_assert!(false);
                }
            }
            Info::LostRuler { player_id, reason } => {
                let get_alias = |id: Option<PlayerId>| {
                    id.and_then(|id| players.borrow_player(id).map(|p| p.alias()))
                };

                let mut player = players.borrow_player_mut(player_id).unwrap();
                player.death_reason = match reason {
                    LostRulerReason::KilledBy(attacker_player_id, unit) => {
                        Some(DeathReason::RulerKilled {
                            alias: get_alias(attacker_player_id),
                            unit,
                        })
                    }
                };
                maybe_dead(player_id);
            }
            Info::LostTower {
                tower_id,
                player_id,
                reason: _,
            } => {
                if let Some(mut old_player) = players.borrow_player_mut(player_id) {
                    let removed = old_player.towers.remove(&tower_id);
                    debug_assert!(removed);
                } else {
                    debug_assert!(false);
                }
            }
            _ => {}
        }
    }
}
