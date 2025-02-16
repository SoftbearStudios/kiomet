// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use common::tower::TowerId;
use kodiak_client::fxhash::FxHashMap;
use kodiak_client::glam::Vec2;
use kodiak_client::PlayerId;
use std::collections::hash_map::Entry;

#[derive(Default)]
pub struct Territories {
    inner: FxHashMap<PlayerId, Territory>,
}

impl Territories {
    /// Record that a visible tower at `tower_id` has a `player_id`.
    #[inline]
    pub fn record(&mut self, tower_id: TowerId, player_id: PlayerId) {
        let t = match self.inner.entry(player_id) {
            Entry::Occupied(e) => e.into_mut(),
            Entry::Vacant(e) => (#[cold]
            || e.insert(Default::default()))(),
        };
        let pos = tower_id.as_vec2();
        t.sum += pos;
        t.count += 1;

        if let Some(center_of_mass) = t.center_of_mass {
            let new = (tower_id, pos.distance_squared(center_of_mass));
            let best = t.best_tower_id.get_or_insert(new);
            if new.1 < best.1 {
                *best = new;
            }
        }
    }

    /// Call each frame after recording all the visible towers. Calls a function for rendering each
    /// territory given it's player, center, and tower count.
    pub fn update(&mut self, elapsed_seconds: f32, mut f: impl FnMut(PlayerId, Vec2, usize)) {
        self.inner.retain(|&player_id, t| {
            // Take data that needs to be recalculated every frame.
            let count = std::mem::take(&mut t.count);
            let sum = std::mem::take(&mut t.sum);
            let best_tower_id = std::mem::take(&mut t.best_tower_id);
            if count == 0 {
                return false;
            }
            t.center_of_mass = Some(sum * (1.0 / count as f32));

            if let Some((tower_id, _)) = best_tower_id {
                let new_pos = tower_id.as_vec2();
                let pos = t.pos.get_or_insert(new_pos);
                let delta = new_pos - *pos;
                *pos += delta.clamp_length_max(elapsed_seconds * (3.0 + delta.length()));

                // Can only render once we have a pos.
                f(player_id, *pos, count);
            }
            true
        })
    }
}

#[derive(Debug, Default)]
struct Territory {
    best_tower_id: Option<(TowerId, f32)>,
    center_of_mass: Option<Vec2>,
    count: usize,
    pos: Option<Vec2>,
    sum: Vec2,
}
