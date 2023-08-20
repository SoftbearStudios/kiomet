// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::service::TowerService;
use common::field::Field;
use common::force::Path;
use common::protocol::Command;
use common::ticks::Ticks;
use common::tower::TowerType;
use common::unit::Unit;
use common::world::World;
use core_protocol::id::PlayerId;
use core_protocol::name::PlayerAlias;
use game_server::game_service::{Bot, BotAction, GameArenaService};
use game_server::player::{PlayerRepo, PlayerTuple};
use rand::prelude::{IteratorRandom, ThreadRng};
use rand::{thread_rng, Rng};
use std::cmp::Ordering;
use std::sync::Arc;

pub struct TowerBot {
    /// Bot will try to accumulate this many towers.
    territorial_ambition: u8,
    /// Time until quit.
    before_quit: Ticks,
    /// War against player, and time remaining.
    war: Option<War>,
}

#[derive(Copy, Clone, Debug)]
struct War {
    against: PlayerId,
    remaining: Ticks,
}

impl TowerBot {
    fn random_before_quit(rng: &mut ThreadRng) -> Ticks {
        Ticks::from_whole_secs(if false {
            rng.gen_range(0..=5)
        } else if cfg!(debug_assertions) && rng.gen_bool(0.1) {
            rng.gen_range(80..=120)
        } else {
            rng.gen_range(1800..=5400)
        })
    }
}

impl Default for TowerBot {
    fn default() -> Self {
        let mut rng = thread_rng();
        Self {
            territorial_ambition: rng.gen_range(8..=12),
            before_quit: Self::random_before_quit(&mut rng),
            war: None,
        }
    }
}

pub struct Input<'a> {
    world: &'a World,
}

impl Bot<TowerService> for TowerBot {
    const DEFAULT_MIN_BOTS: usize = 10;
    const DEFAULT_BOT_PERCENT: usize = 80;

    type Input<'a> = Option<Input<'a>>;

    fn get_input<'a>(
        service: &'a TowerService,
        player_tuple: &'a Arc<PlayerTuple<TowerService>>,
        _players: &'a PlayerRepo<TowerService>,
    ) -> Self::Input<'a> {
        service
            .regulator
            .active(player_tuple.borrow_player().player_id)
            .then_some(Input {
                world: &service.world,
            })
    }

    fn update<'a>(
        &mut self,
        input: Self::Input<'_>,
        player_id: PlayerId,
        players: &'a PlayerRepo<TowerService>,
    ) -> BotAction<<TowerService as GameArenaService>::GameRequest> {
        let Some(input) = input else {
            return BotAction::None("no input");
        };
        let player = match players.borrow_player(player_id) {
            Some(player) => player,
            None => return BotAction::Quit,
        };

        let mut rng = thread_rng();

        if !player.alive {
            self.war = None;
            self.before_quit = Self::random_before_quit(&mut rng);
            return BotAction::Some(Command::Spawn);
        }

        let Some((random_tower_id, random_tower))
            = player
                .towers
                .iter()
                .filter_map(|&tower_id|
                    input
                        .world
                        .chunk
                        .get(tower_id)
                        .filter(|tower| !tower.force_units().is_empty())
                        .map(|tower| (tower_id, tower))
                )
                .choose(&mut rng) else {
            // Don't crash if ruler is on the run and enemy is hot on it's tail.
            return BotAction::None("no towers");
        };

        let world_player = input.world.player(player_id);

        if let Some(before_quit) = self.before_quit.checked_sub(Ticks::ONE) {
            self.before_quit = before_quit;
        } else {
            // We are close to the world center, so leave and make room for real players.
            println!("bot quitting with {} towers", player.towers.len());
            return BotAction::Quit;
        };

        // Expire the war eventually.
        self.war = self.war.and_then(|war| {
            war.remaining
                .checked_sub(Ticks::ONE)
                .filter(|_| {
                    players
                        .borrow_player(war.against)
                        .map_or(false, |against| against.alive)
                })
                .map(|remaining| War {
                    against: war.against,
                    remaining,
                })
        });

        // Check if can upgrade. Require more shield if in war.
        let min_shield = random_tower.tower_type.raw_unit_capacity(Unit::Shield)
            / (1 + self.war.is_none() as usize);

        if random_tower.units.available(Unit::Shield) >= min_shield {
            if let Some(tower_type) = random_tower
                .tower_type
                .upgrades()
                .filter(|u| {
                    u.has_prerequisites(&player.tower_counts) && !matches!(u, TowerType::Helipad)
                })
                .choose(&mut rng)
            {
                return BotAction::Some(Command::Upgrade {
                    tower_id: random_tower_id,
                    tower_type,
                });
            }
        }

        // Recompute war.
        if self.war.is_none() && rng.gen_bool(0.005) {
            #[derive(PartialEq)]
            struct WarTarget {
                player_id: PlayerId,
                alias: PlayerAlias,
                towers: u32,
            }

            impl PartialOrd for WarTarget {
                fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                    Some(self.towers.cmp(&other.towers))
                }
            }

            let mut best_target = Option::<WarTarget>::None;
            for (_, tower) in input.world.chunk.iter_towers_square(random_tower_id, 5) {
                if let Some(enemy_id) = tower.player_id {
                    if enemy_id == player_id || enemy_id.is_bot() {
                        // Not enemy.
                        continue;
                    }

                    if let Some(enemy) = players.borrow_player(enemy_id) {
                        if enemy.towers.len() / 8 <= self.territorial_ambition as usize {
                            continue;
                        }

                        let target = Some(WarTarget {
                            player_id: enemy_id,
                            alias: enemy.alias(),
                            towers: enemy.towers.len() as u32,
                        });
                        if target > best_target {
                            best_target = target;
                        }
                    }
                }
            }
            if let Some(best_target) = best_target {
                println!(
                    "BOT {} ({:?}) declaring WAR on {} ({:?})",
                    player.alias(),
                    player_id,
                    best_target.alias,
                    best_target.player_id
                );
                self.war = Some(War {
                    against: best_target.player_id,
                    remaining: Ticks::from_whole_secs(if best_target.towers > 500 {
                        360
                    } else {
                        180
                    }),
                });
                if world_player.allies.contains(&best_target.player_id) {
                    return BotAction::Some(Command::Alliance {
                        with: best_target.player_id,
                        break_alliance: true,
                    });
                }
            }
        }

        // Contemplate entering an alliance.
        if rng.gen_bool(0.0025) {
            let with = input
                .world
                .chunk
                .iter_towers_square(random_tower_id, 4)
                .find_map(|(_, candidate_destination_tower)| {
                    candidate_destination_tower
                        .player_id
                        .and_then(|player_id| players.borrow_player(player_id))
                        .filter(|player| {
                            /* !player.is_bot()
                            && */
                            player.towers.len() / 8 <= self.territorial_ambition as usize
                                && ((player.player_id.0.get() ^ player_id.0.get()) & 0b1 == 0)
                                && !world_player.allies.contains(&player.player_id)
                        })
                        .map(|player| player.player_id)
                });
            if let Some(with) = with {
                return BotAction::Some(Command::Alliance {
                    with,
                    break_alliance: false,
                });
            }
        }

        // Contemplate dispatching a force.
        let strength = random_tower.force_units();
        if !strength.is_empty() {
            // Whether ruler would be part of force.
            let sending_ruler = strength.contains(Unit::Ruler);

            // Whether this force will do significant damage as opposed to bouncing.
            let formidable = {
                let mut total_damage = 0u32;
                for unit_damage in strength.iter().map(|(unit, count)| {
                    Unit::damage_to_finite(
                        unit.damage(unit.field(false, true, false), Field::Surface),
                    )
                    .saturating_mul(count as u32)
                }) {
                    total_damage = total_damage.saturating_add(unit_damage);
                }
                total_damage >= 5
            };

            let destination = input
                .world
                .chunk
                .iter_towers_square(random_tower_id, 5)
                .filter(|&(_, candidate_destination_tower)| {
                    if candidate_destination_tower.player_id == Some(player_id) {
                        // Can shuffle units if not at war or to protect ruler while at war.
                        self.war.is_none()
                            || (sending_ruler
                                && candidate_destination_tower.player_id.is_some()
                                && candidate_destination_tower.units.available(Unit::Shield)
                                    > Unit::damage_to_finite(
                                        candidate_destination_tower
                                            .tower_type
                                            .max_ranged_damage(),
                                    ) as usize)
                    } else if sending_ruler
                        || candidate_destination_tower
                            .player_id
                            .map(|p|
                                input
                                    .world
                                    .player(p)
                                    .allies
                                    .contains(&player_id)
                                && world_player.allies.contains(&p)
                            ).unwrap_or(false) {
                        // Cannot send ruler to an unowned tower or forces to an allied tower.
                        false
                    } else if let Some(War { against, .. }) = self.war {
                        // Focus on the adversary (or securing more unclaimed towers).
                        (formidable && candidate_destination_tower.player_id == Some(against)) || candidate_destination_tower.player_id.is_none()
                    } else {
                        candidate_destination_tower
                            .player_id
                            .and_then(|player_id| players.borrow_player(player_id))
                            .map_or(
                                true, // player.towers.len() < self.territorial_ambition as usize,
                                |enemy| {
                                    // They're big; get em!
                                    enemy.towers.len() / 4 > self.territorial_ambition as usize
                                        // They're big; get em!
                                        || enemy.score > 1000
                                        // Don't do too much damage to smol's.
                                        || !formidable
                                        // Recently changed hands?
                                        || candidate_destination_tower.units.available(Unit::Shield) < 5
                                },
                            )
                    }
                })
                .choose(&mut rng);

            if let Some((destination, _)) = destination {
                let max_edge_distance = strength.max_edge_distance();
                let path = input.world.find_best_path(
                    random_tower_id,
                    destination,
                    max_edge_distance,
                    player_id,
                    |_| true,
                );

                if let Some(path) = path {
                    return BotAction::Some(
                        if sending_ruler
                            || !random_tower.generates_mobile_units()
                            || rng.gen_bool(0.75)
                        {
                            Command::deploy_force_from_path(path)
                        } else {
                            Command::SetSupplyLine {
                                tower_id: path[0],
                                path: Some(Path::new(path)),
                            }
                        },
                    );
                } else {
                    return BotAction::None("no path");
                }
            } else {
                return BotAction::None("no destination");
            }
        } else {
            return BotAction::None("empty force");
        }

        //BotAction::None("no action")
    }
}
