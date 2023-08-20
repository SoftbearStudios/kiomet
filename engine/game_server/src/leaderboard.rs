// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::game_service::GameArenaService;
use crate::infrastructure::Infrastructure;
use crate::liveboard::LiveboardRepo;
use crate::player::PlayerRepo;
use core_protocol::dto::LeaderboardScoreDto;
use core_protocol::id::PeriodId;
use core_protocol::name::PlayerAlias;
use core_protocol::rpc::LeaderboardUpdate;
use core_protocol::PlasmaRequestV1;
use server_util::rate_limiter::RateLimiter;
use std::collections::HashMap;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

/// Manages updating, saving, and loading leaderboards.
pub struct LeaderboardRepo<G: GameArenaService> {
    /// Stores cached leaderboards from database and whether they were changed.
    leaderboards: [(Arc<[LeaderboardScoreDto]>, bool); std::mem::variant_count::<PeriodId>()],
    /// Scores that should be committed to database.
    pending: HashMap<PlayerAlias, u32>,
    take_pending_rate_limit: RateLimiter,
    _spooky: PhantomData<G>,
}

impl<G: GameArenaService> Default for LeaderboardRepo<G> {
    fn default() -> Self {
        Self {
            leaderboards: [
                (Vec::new().into(), false),
                (Vec::new().into(), false),
                (Vec::new().into(), false),
            ],
            pending: HashMap::new(),
            take_pending_rate_limit: RateLimiter::new(Duration::from_secs(60), 0),
            _spooky: PhantomData,
        }
    }
}

impl<G: GameArenaService> LeaderboardRepo<G> {
    /// Gets a cached leaderboard.
    pub fn get(&self, period_id: PeriodId) -> &Arc<[LeaderboardScoreDto]> {
        &self.leaderboards[period_id as usize].0
    }

    /// Leaderboard relies on an external source of data, such as a database.
    pub fn put_leaderboard(
        &mut self,
        period_id: PeriodId,
        leaderboard: Box<[LeaderboardScoreDto]>,
    ) {
        let leaderboard: Arc<[LeaderboardScoreDto]> = Vec::from(leaderboard).into();
        if &leaderboard != self.get(period_id) {
            self.leaderboards[period_id as usize] = (leaderboard, true);
        }
    }

    /// Computes minimum score to earn a place on the given leaderboard.
    fn minimum_score(&self, period_id: PeriodId) -> u32 {
        self.get(period_id)
            .get(G::LEADERBOARD_SIZE - 1)
            .map(|dto| dto.score)
            .unwrap_or(1)
    }

    /// Process liveboard scores to potentially be added to the leaderboard.
    pub(crate) fn process(&mut self, liveboard: &LiveboardRepo<G>, players: &PlayerRepo<G>) {
        let liveboard_items = liveboard.get();

        // Must be sorted in reverse.
        debug_assert!(liveboard_items.is_sorted_by_key(|dto| u32::MAX - dto.score));

        #[cfg(not(debug_assertions))]
        if players.real_players_live < G::LEADERBOARD_MIN_PLAYERS {
            return;
        }

        for dto in liveboard_items.iter() {
            if PeriodId::iter().all(|period_id| dto.score < self.minimum_score(period_id)) {
                // Sorted, so this iteration is not going to produce any more sufficient scores.
                break;
            }

            if dto.player_id.is_bot() {
                // Bots are never on the leaderboard, regardless of whether they are on the liveboard.
                continue;
            }

            if let Some(player) = players.borrow_player(dto.player_id) {
                let alias = player.alias();
                let entry = self.pending.entry(alias).or_insert(0);
                *entry = dto.score.max(*entry);
            } else {
                // TODO: Is this legitimately possible?
                debug_assert!(false, "player from liveboard doesn't exist");
            }
        }
    }

    /// Returns scores pending database commit, draining them in the process. Rate limited.
    pub fn take_pending(&mut self) -> Option<Box<[LeaderboardScoreDto]>> {
        if self.pending.is_empty() || self.take_pending_rate_limit.should_limit_rate() {
            None
        } else {
            Some(
                self.pending
                    .drain()
                    .map(|(alias, score)| LeaderboardScoreDto { alias, score })
                    .collect(),
            )
        }
    }

    pub fn update_to_plasma(infrastructure: &mut Infrastructure<G>) {
        for (realm_name, context_service) in infrastructure.arenas.iter_mut() {
            if let Some(scores) = context_service.context.leaderboard.take_pending() {
                infrastructure
                    .plasma
                    .do_request(PlasmaRequestV1::UpdateLeaderboards {
                        game_id: G::GAME_ID,
                        server_id: infrastructure.server_id,
                        realm_name,
                        scores,
                    });
            }
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (PeriodId, &Arc<[LeaderboardScoreDto]>)> {
        self.leaderboards
            .iter()
            .enumerate()
            .map(|(i, (leaderboard, _))| (PeriodId::from(i), leaderboard))
    }

    /// Reads off changed leaderboards, *without* the changed flag in the process.
    pub fn deltas_nondestructive(
        &self,
    ) -> impl Iterator<Item = (PeriodId, &Arc<[LeaderboardScoreDto]>)> {
        self.leaderboards
            .iter()
            .enumerate()
            .filter_map(|(i, (leaderboard, changed))| {
                if *changed {
                    Some((PeriodId::from(i), leaderboard))
                } else {
                    None
                }
            })
    }

    /// Clear all the delta flags (such as if clients have been updated).
    pub fn clear_deltas(&mut self) {
        for (_, changed) in self.leaderboards.iter_mut() {
            *changed = false;
        }
    }

    /// Gets leaderboard for new players.
    pub fn initializers(&self) -> impl Iterator<Item = LeaderboardUpdate> + '_ {
        self.iter().filter_map(|(period_id, leaderboard)| {
            if leaderboard.is_empty() {
                None
            } else {
                Some(LeaderboardUpdate::Updated(
                    period_id,
                    Arc::clone(leaderboard),
                ))
            }
        })
    }
}
