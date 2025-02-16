// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::game::is_visible;
use crate::path::{PathId, PathLayer};
use crate::KiometGame;
use common::alerts::AlertFlag;
use common::force::Path;
use common::tower::{Tower, TowerId, TowerType};
use deployment::{best_deployment, still_is_deployment};
use kodiak_client::glam::{Vec2, Vec3};
use kodiak_client::{ClientContext, PlayerId};
use std::f32::consts::PI;
use upgrade::{best_upgrade, still_is_upgrade};

pub enum Tutorial {
    /// Waiting for path to suggest.
    WaitingToDeploy,
    Deploying {
        path: Path,
        start: f32,
    },
    WaitingToUpgrade,
    Upgrading {
        tower_id: TowerId,
        start: f32,
    },
    Done,
}

impl Default for Tutorial {
    fn default() -> Self {
        Self::WaitingToDeploy
    }
}

#[derive(Debug, Copy, Clone, PartialEq)]
pub enum TutorialAlert {
    Capture(TowerId),
    Upgrade(TowerId),
    /// Tutorial not over yet, but not immediate action.
    Pending,
}

impl Tutorial {
    pub fn alert(&self) -> Option<TutorialAlert> {
        Some(match self {
            Self::Deploying { path, .. } => TutorialAlert::Capture(path.iter().last().unwrap()),
            Self::Upgrading { tower_id, .. } => TutorialAlert::Upgrade(*tower_id),
            Self::Done => return None,
            _ => TutorialAlert::Pending,
        })
    }

    pub fn dismiss_capture(&mut self) {
        if matches!(self, Self::WaitingToDeploy { .. } | Self::Deploying { .. }) {
            *self = Self::WaitingToUpgrade
        }
    }

    pub fn dismiss_upgrade(&mut self) {
        *self = Self::Done;
    }

    /// Only checks context's game state for changes.
    pub fn update(&mut self, context: &ClientContext<KiometGame>) {
        if context.state.game.alive {
            if context.state.game.bounding_rectangle.area() > 18u32.pow(2) {
                // Don't gank the performance by pathfinding too much.
                *self = Self::Done;
            }

            let flags = context.state.game.alerts.flags();
            match self {
                &mut Self::WaitingToDeploy => {
                    if flags.contains(AlertFlag::DeployedAnyForce | AlertFlag::UpgradedAnyTower) {
                        // Probably refreshed page into an existing base, skip the tutorial.
                        *self = Self::Done;
                    } else if let Some(tower_id) = best_upgrade(context)
                        .filter(|_| flags.contains(AlertFlag::DeployedAnyForce))
                    {
                        *self = Self::Upgrading {
                            tower_id,
                            start: context.client.time_seconds,
                        };
                    } else if let Some(path) = best_deployment(context) {
                        *self = Self::Deploying {
                            path,
                            start: context.client.time_seconds,
                        }
                    }
                }
                Self::Deploying { path, .. } => {
                    if !still_is_deployment(context, path) {
                        *self = Self::WaitingToDeploy;
                        self.update(context); // Update right away.
                    }
                }
                Self::WaitingToUpgrade => {
                    if flags.contains(AlertFlag::UpgradedAnyTower) {
                        *self = Self::Done;
                    } else if let Some(tower_id) = best_upgrade(context) {
                        *self = Self::Upgrading {
                            tower_id,
                            start: context.client.time_seconds,
                        };
                    }
                }
                Self::Upgrading { tower_id, .. } => {
                    if flags.contains(AlertFlag::UpgradedAnyTower) {
                        *self = Self::Done;
                    } else if !still_is_upgrade(context, *tower_id) {
                        *self = Self::WaitingToUpgrade;
                        self.update(context); // Update right away.
                    }
                }
                Self::Done => {}
            }
        } else {
            *self = Self::default();
        }
    }

    pub fn render(
        &self,
        path_layer: &mut PathLayer,
        selected_tower_id: Option<TowerId>,
        time: f32,
    ) {
        const SCALE: f32 = 1.6;

        // t of 0 is up and 1 is down.
        let mut draw_cursor = |pointer: Vec2, t: f32, fade_in: bool| {
            let fade_t = fade_in
                .then(|| (t * 4.0 - 0.5).clamp(0.0, 1.0))
                .unwrap_or(1.0);
            let down_t = (t * 4.0 - 2.0).clamp(0.0, 1.0);
            let circle_t = (t * 4.0 - 3.0).clamp(0.0, 1.0);

            // scale, brightness
            let [scale, brightness] = (Vec2::new(1.0, 1.0).lerp(Vec2::new(0.85, 0.75), down_t)
                * Vec2::new(fade_t, fade_t * 0.3 + 0.7))
            .to_array();

            let stroke = (Vec3::splat(0.89) * brightness).extend(1.0);
            let fill = (Vec3::splat(0.73) * brightness).extend(1.0);

            path_layer.draw_path_a(
                PathId::Cursor,
                pointer,
                0.0,
                SCALE * scale,
                Some(stroke),
                Some(fill),
                false,
            );

            if circle_t != 0.0 {
                path_layer.draw_path_a(
                    PathId::Explosion,
                    pointer,
                    0.0,
                    SCALE * 0.16 * circle_t,
                    None,
                    Some(Vec3::splat(0.4).extend(0.4)),
                    false,
                );
            }
        };

        match self {
            Self::Deploying { path, start, .. } => {
                const FADE: f32 = 1.0;
                const MOVE: f32 = 2.5;
                const PERIOD: f32 = FADE * 2.0 + MOVE;
                let time = (time - start) % PERIOD;
                if time <= FADE {
                    let t = time / FADE;
                    draw_cursor(path.source().as_vec2(), t, true);
                } else if time <= PERIOD - FADE {
                    let progress = (time - FADE) / (PERIOD - 2.0 * FADE);
                    let segments = path.iter().count() - 1;
                    let segment_index_f32 = progress * segments as f32;
                    let segment_index = (segment_index_f32.floor() as usize).min(segments - 1);
                    let source = path.iter().nth(segment_index).unwrap();
                    let destination = path.iter().nth(segment_index + 1).unwrap();

                    draw_cursor(
                        source
                            .as_vec2()
                            .lerp(destination.as_vec2(), segment_index_f32.fract()),
                        1.0,
                        true,
                    );
                } else {
                    let t = (PERIOD - time) / FADE;
                    draw_cursor(path.destination().as_vec2(), t, true);
                }
            }
            Self::Upgrading { tower_id, start } if selected_tower_id != Some(*tower_id) => {
                let t = ((time - start) * PI * 0.45).sin().abs();
                draw_cursor(tower_id.as_vec2(), t, false);
            }
            _ => {}
        }
    }
}

mod deployment {
    use std::num::NonZeroU16;

    use super::*;

    /// Returns the best deployment option found.
    pub fn best_deployment(context: &ClientContext<KiometGame>) -> Option<Path> {
        // Don't consider more than 50 options because A* is expensive.
        iter_deployments(context)
            .take(50)
            .max_by_key(|(path, tower_type)| {
                let mut score = 0;
                score -= (path.iter().count() * 4) as i32;
                if tower_type.generates_mobile_units() {
                    // Helps with offense/defense.
                    score += 2;
                }
                if context.state.game.tower_counts[*tower_type] == 0 {
                    // Helps with upgrades.
                    score += 1
                }
                score
            })
            .map(|(path, _)| path)
    }

    /// Returns if a deployment previously returned by [`Self::best_deployment`] is valid.
    pub fn still_is_deployment(context: &ClientContext<KiometGame>, path: &Path) -> bool {
        (|| {
            let src_id = path.source();
            let src = context.state.game.world.chunk.get(src_id)?;
            if !filter_deployment_src(context, src) {
                return None;
            }

            let dst_id = path.destination();
            let dst = context.state.game.world.chunk.get(dst_id)?;
            if !filter_deployment_dst(dst) {
                return None;
            }

            (find_best_deployment_path(context, src_id, dst_id).as_ref() == Some(path))
                .then_some(())
        })()
        .is_some()
    }

    /// Is ours and has at least 1 useful unit.
    fn filter_deployment_src(context: &ClientContext<KiometGame>, src: &Tower) -> bool {
        src.player_id == context.state.core.player_id
            && !src.units.has_ruler()
            && src.units.iter().any(|(unit, _)| unit.can_capture())
    }

    /// Has to be available, not occupied by zombies and not contested.
    fn filter_deployment_dst(dst: &Tower) -> bool {
        dst.player_id.is_none() && dst.units.is_empty() && dst.inbound_forces.is_empty()
    }

    /// Finds the best valid path to a deployment.
    fn find_best_deployment_path(
        context: &ClientContext<KiometGame>,
        src_id: TowerId,
        dst_id: TowerId,
    ) -> Option<Path> {
        context
            .state
            .game
            .world
            .find_best_path(
                src_id,
                dst_id,
                None,
                context
                    .state
                    .core
                    .player_id
                    .unwrap_or(PlayerId(NonZeroU16::MAX)),
                |tower_id| {
                    is_visible(context, tower_id)
                        && (tower_id == src_id
                            || context
                                .state
                                .game
                                .world
                                .chunk
                                .get(tower_id)
                                .map(|tower| {
                                    tower.player_id == context.state.core.player_id
                                        || filter_deployment_dst(tower)
                                })
                                .unwrap_or(false))
                },
            )
            .map(|path| Path::new(path))
    }

    fn iter_deployments(
        context: &ClientContext<KiometGame>,
    ) -> impl Iterator<Item = (Path, TowerType)> + '_ {
        context
            .state
            .game
            .world
            .chunk
            .iter_towers()
            .filter(|(_, src)| filter_deployment_src(context, src))
            .flat_map(move |(src_id, _)| {
                context
                    .state
                    .game
                    .world
                    .chunk
                    .iter_towers_square(src_id, 3)
                    .filter_map(move |(dst_id, dst)| {
                        filter_deployment_dst(dst)
                            .then(|| {
                                find_best_deployment_path(context, src_id, dst_id)
                                    .map(|path| (path, dst.tower_type))
                            })
                            .flatten()
                    })
            })
    }
}

mod upgrade {
    use super::*;

    /// Returns the best upgrade option found.
    pub fn best_upgrade(context: &ClientContext<KiometGame>) -> Option<TowerId> {
        iter_upgrades(context)
            .max_by_key(|(_, upgrade)| upgrade.generates_mobile_units())
            .map(|(tower_id, _)| tower_id)
    }

    /// Returns if a deployment previously returned by [`Self::best_upgrade`] is valid.
    pub fn still_is_upgrade(context: &ClientContext<KiometGame>, tower_id: TowerId) -> bool {
        (|| {
            let tower = context.state.game.world.chunk.get(tower_id)?;
            iter_tower_upgrades(context, tower_id, tower)
                .next()
                .filter(|_| tower.player_id == context.player_id())
        })()
        .is_some()
    }

    /// Iterates upgrades of a tower that are available.
    fn iter_tower_upgrades<'a>(
        context: &'a ClientContext<KiometGame>,
        tower_id: TowerId,
        tower: &'a Tower,
    ) -> impl Iterator<Item = (TowerId, TowerType)> + 'a {
        tower.tower_type.upgrades().filter_map(move |upgrade| {
            (tower.delay.is_none() && upgrade.has_prerequisites(&context.state.game.tower_counts))
                .then_some((tower_id, upgrade))
        })
    }

    fn iter_upgrades(
        context: &ClientContext<KiometGame>,
    ) -> impl Iterator<Item = (TowerId, TowerType)> + '_ {
        context
            .state
            .game
            .world
            .chunk
            .iter_towers()
            .filter(|(_, tower)| tower.player_id == context.state.core.player_id)
            .flat_map(|(id, t)| iter_tower_upgrades(context, id, t))
    }
}
