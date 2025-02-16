// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::service::TowerService;
use common::field::Field;
use common::force::Path;
use common::protocol::Command;
use common::ticks::Ticks;
use common::tower::{TowerId, TowerType};
use common::unit::Unit;
use kodiak_server::rand::prelude::{IteratorRandom, ThreadRng};
use kodiak_server::rand::{thread_rng, Rng};
use kodiak_server::{
    random_bot_name, ArenaService, ArenaSettingsDto, Bot, BotAction, BotOptions, Player,
    PlayerAlias, PlayerId,
};
use std::cmp::Ordering;

#[derive(Debug)]
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
    focus: TowerId,
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

impl Bot<TowerService> for TowerBot {
    const AUTO: BotOptions = BotOptions {
        min_bots: 10,
        max_bots: 200,
        bot_percent: 80,
    };

    fn update<'a>(
        input: &TowerService,
        player_id: PlayerId,
        player: &'a mut Player<TowerService>,
        settings: &ArenaSettingsDto<<TowerService as ArenaService>::ArenaSettings>,
    ) -> BotAction<<TowerService as ArenaService>::GameRequest> {
        let mut rng = thread_rng();
        let bot = player.inner.bot_mut().unwrap();
        let player = &input.player_data[player_id];

        if !player.alive {
            bot.war = None;
            bot.before_quit = Self::random_before_quit(&mut rng);
            return BotAction::Some(Command::Spawn(random_bot_name()));
        }

        let Some((random_tower_id, random_tower)) = player
            .towers
            .iter()
            .filter_map(|&tower_id| {
                input
                    .world
                    .chunk
                    .get(tower_id)
                    .filter(|tower| !tower.force_units().is_empty())
                    .map(|tower| (tower_id, tower))
            })
            .choose(&mut rng)
        else {
            // Don't crash if ruler is on the run and enemy is hot on it's tail.
            return BotAction::None("no towers");
        };

        let world_player = input.world.player(player_id);

        if let Some(before_quit) = bot.before_quit.checked_sub(Ticks::ONE) {
            bot.before_quit = before_quit;
        } else {
            // We are close to the world center, so leave and make room for real players.
            println!("bot quitting with {} towers", player.towers.len());
            return BotAction::Quit;
        };

        // Expire the war eventually.
        bot.war = bot.war.and_then(|war| {
            war.remaining
                .checked_sub(Ticks::ONE)
                .filter(|_| {
                    input
                        .player_data
                        .get(war.against)
                        .map_or(false, |against| against.alive)
                })
                .map(|remaining| War {
                    against: war.against,
                    remaining,
                    focus: war.focus,
                })
        });

        // Check if can upgrade. Require more shield if in war.
        let min_shield = random_tower.tower_type.raw_unit_capacity(Unit::Shield)
            / (1 + bot.war.is_none() as usize);

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
        if bot.war.is_none() && rng.gen_bool(0.005) {
            #[derive(PartialEq)]
            struct WarTarget {
                player_id: PlayerId,
                alias: PlayerAlias,
                towers: u32,
                focus: TowerId,
            }

            impl PartialOrd for WarTarget {
                fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
                    Some(self.towers.cmp(&other.towers))
                }
            }

            let mut best_target = Option::<WarTarget>::None;
            for (tower_id, tower) in input.world.chunk.iter_towers_square(random_tower_id, 5) {
                if let Some(enemy_id) = tower.player_id {
                    if enemy_id == player_id || enemy_id.is_bot() {
                        // Not enemy.
                        continue;
                    }

                    if let Some(enemy) = input.player_data.get(enemy_id) {
                        if enemy.towers.len() / 8 <= bot.territorial_ambition as usize {
                            continue;
                        }

                        let target = Some(WarTarget {
                            player_id: enemy_id,
                            alias: enemy.alias,
                            towers: enemy.towers.len() as u32,
                            focus: tower_id,
                        });
                        if target > best_target {
                            best_target = target;
                        }
                    }
                }
            }
            if let Some(best_target) = best_target {
                /*
                println!(
                    "BOT {} ({:?}) declaring WAR on {} ({:?})",
                    player.alias, player_id, best_target.alias, best_target.player_id
                );
                */
                bot.war = Some(War {
                    against: best_target.player_id,
                    remaining: Ticks::from_whole_secs(if best_target.towers > 500 {
                        360
                    } else {
                        180
                    }) * settings.bot_aggression(),
                    focus: best_target.focus,
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
                        .and_then(|id| input.player_data.get(id).map(|p| (id, p)))
                        .filter(|(id, p)| {
                            // PlayerId's 2nd to LSB is ~random (the LSB is dependent on player vs. bot)
                            p.towers.len() / 8 <= bot.territorial_ambition as usize
                                && ((id.0.get() ^ player_id.0.get()) & 0b10 == 0)
                                && !world_player.allies.contains(&id)
                        })
                        .map(|(id, _)| id)
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

            if formidable || sending_ruler || rng.gen_bool(0.01) {
                let reusable = strength.iter().any(|(u, _)| !u.is_single_use());
                let destination = input
                    .world
                    .chunk
                    .iter_towers_square(random_tower_id, 5)
                    .filter(|&(candidate_destination_tower_id, candidate_destination_tower)| {
                        if candidate_destination_tower.player_id == Some(player_id) {
                            let can_send_ruler = candidate_destination_tower.units.available(Unit::Shield)
                                > Unit::damage_to_finite(
                                    candidate_destination_tower
                                        .tower_type
                                        .max_ranged_damage(),
                                ) as usize;

                            // Can shuffle units if:
                            // - not at war
                            // - towards the front lines XOR sending ruler
                            // - to protect ruler
                            reusable && bot
                                .war
                                .map(|war| sending_ruler ^ (war.focus.distance_squared(candidate_destination_tower_id) < war.focus.distance_squared(random_tower_id)))
                                .unwrap_or(true)
                                && !(sending_ruler && !can_send_ruler)
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
                                ).unwrap_or(false)
                            || (!reusable && candidate_destination_tower.player_id.is_none()) {
                            // Cannot send:
                            // - ruler to an unowned tower
                            // - forces to an allied tower
                            // - single-use units to an unclaimed tower
                            false
                        } else if strength.contains(Unit::Nuke) && candidate_destination_tower
                            .player_id
                            .and_then(|player_id| input.player_data.get(player_id))
                            .map_or(
                                false,
                                |enemy| {
                                    enemy.score < 2000
                                }
                            ) {
                            // No nuking smol's.
                            false
                        } else if let Some(War { against, .. }) = bot.war {
                            // Focus on the adversary (or securing more unclaimed towers).
                            (formidable && candidate_destination_tower.player_id == Some(against)) || candidate_destination_tower.player_id.is_none()
                        } else {
                            candidate_destination_tower
                                .player_id
                                .and_then(|player_id| input.player_data.get(player_id))
                                .map_or(
                                    true,
                                    |enemy| {
                                        // They're big; get em!
                                        enemy.towers.len() / 4 > bot.territorial_ambition as usize
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

                if let Some((destination, destination_tower)) = destination {
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
                                || !destination_tower.generates_mobile_units()
                                || rng.gen_bool(0.8)
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
                return BotAction::None("not formidable");
            }
        } else {
            return BotAction::None("empty force");
        }

        //BotAction::None("no action")
    }
}
