// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::{game::KiometGame, settings::Unlocks};
use common::tower::{Tower, TowerId};
use kodiak_client::{ClientContext, RankNumber};

pub struct KeyDispenser {
    last_key_time: f32,
    key: Option<TowerId>,
}

impl Default for KeyDispenser {
    fn default() -> Self {
        Self {
            last_key_time: 60.0 - Self::INTERVAL,
            key: None,
        }
    }
}

impl KeyDispenser {
    /// How long keys last.
    const DURATION: f32 = 250.0;
    /// How often keys spawn.
    const INTERVAL: f32 = 300.0;

    /// Returns key and opacity.
    pub fn key(&self, time: f32) -> Option<(TowerId, f32)> {
        self.key
            .map(|key| {
                (key, {
                    let progress = self.progress(time, Self::DURATION);
                    if progress < 0.5 {
                        1.0
                    } else {
                        1.0 - (progress - 0.5) * 2.0
                    }
                })
            })
            .filter(|(_, opacity)| *opacity > 0.0)
    }

    /// 0..=1 to expiry.
    pub fn progress(&self, time: f32, towards: f32) -> f32 {
        let elapsed = time - self.last_key_time;
        (elapsed / towards).clamp(0.0, 1.0)
    }

    /// Returns if earned the key.
    pub fn update(&mut self, context: &ClientContext<KiometGame>) -> bool {
        if self
            .key
            .and_then(|tower_id| context.state.game.world.chunk.get(tower_id))
            .map(|tower| tower.player_id.is_some() && tower.player_id == context.player_id())
            .unwrap_or(false)
        {
            self.key = None;
            true
        } else {
            if self.progress(context.client.time_seconds, Self::INTERVAL) == 1.0 {
                if context.settings.unlocks.keys >= Unlocks::MAX
                    || context.state.core.rank().flatten() >= Some(RankNumber::Rank3)
                {
                    self.key = None;
                } else {
                    self.last_key_time = context.client.time_seconds;
                    use kodiak_client::rand::prelude::IteratorRandom;
                    self.key = Self::iter_keys(context)
                        .choose(&mut kodiak_client::rand::thread_rng())
                        .map(|(id, _)| id);
                }
            }
            false
        }
    }

    fn iter_keys(context: &ClientContext<KiometGame>) -> impl Iterator<Item = (TowerId, &Tower)> {
        context
            .state
            .game
            .world
            .chunk
            .iter_towers()
            .filter(|(tower_id, _)| Self::can_have_key(*tower_id, context))
    }

    fn can_have_key(tower_id: TowerId, context: &ClientContext<KiometGame>) -> bool {
        if !context.state.game.visible.contains(tower_id) {
            return false;
        }
        let Some(player_id) = context.player_id() else {
            return false;
        };
        let Some(player) = context.state.game.world.player.get(player_id) else {
            // Maybe dead?
            return false;
        };
        let player = &player.actor;
        let Some(tower) = context.state.game.world.chunk.get(tower_id) else {
            return false;
        };
        tower
            .player_id
            .map(|tower_player_id| {
                tower_player_id != player_id && !player.allies.contains(&tower_player_id)
            })
            .unwrap_or(true)
    }
}
