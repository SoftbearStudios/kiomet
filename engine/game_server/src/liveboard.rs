// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::game_service::GameArenaService;
use crate::player::PlayerRepo;
#[cfg(feature = "teams")]
use crate::team::TeamRepo;
use crate::util::diff_small_n;
use core_protocol::dto::{LiveboardDto, YourScoreDto};
use core_protocol::id::PlayerId;
use core_protocol::rpc::LiveboardUpdate;
use server_util::rate_limiter::RateLimiter;
use std::collections::HashMap;
use std::convert::TryInto;
use std::marker::PhantomData;
use std::sync::Arc;
use std::time::Duration;

pub struct LiveboardDelta(
    Arc<[LiveboardDto]>,
    Arc<[PlayerId]>,
    Arc<HashMap<PlayerId, YourScoreDto>>,
);

impl LiveboardDelta {
    pub fn cloned(
        &self,
        player_id: &PlayerId,
    ) -> Option<(Arc<[LiveboardDto]>, Arc<[PlayerId]>, Option<YourScoreDto>)> {
        let your_score = self.2.get(player_id).cloned();
        if !self.0.is_empty() || !self.1.is_empty() || your_score.is_some() {
            Some((Arc::clone(&self.0), Arc::clone(&self.1), your_score))
        } else {
            None
        }
    }
}

/// Manages the live leaderboard of an arena.
pub struct LiveboardRepo<G: GameArenaService> {
    /// Stores previous liveboard for diffing.
    previous: Vec<LiveboardDto>,
    /// The most recently computed leaders and rankings.
    leaders: Arc<[LiveboardDto]>,
    rankings: Arc<HashMap<PlayerId, YourScoreDto>>,
    update_rate_limiter: RateLimiter,
    _spooky: PhantomData<G>,
}

impl<G: GameArenaService> Default for LiveboardRepo<G> {
    fn default() -> Self {
        Self {
            leaders: Vec::new().into(),
            previous: Vec::new(),
            rankings: HashMap::new().into(),
            update_rate_limiter: RateLimiter::new(Duration::from_secs(1), 0),
            _spooky: PhantomData,
        }
    }
}

impl<G: GameArenaService> LiveboardRepo<G> {
    /// Compute the current liveboard.
    fn compute(
        players: &PlayerRepo<G>,
        #[cfg(feature = "teams")] teams: &TeamRepo<G>,
    ) -> Vec<LiveboardDto> {
        let mut liveboard = Vec::with_capacity(players.len());
        liveboard.extend(players.iter_borrow().filter_map(|player| {
            if !player.is_alive() {
                return None;
            }

            if !G::LIVEBOARD_BOTS && player.is_bot() {
                return None;
            }

            #[cfg(feature = "teams")]
            let team = player.team_id().and_then(|t| teams.get(t));

            #[cfg(feature = "teams")]
            debug_assert_eq!(player.team_id().is_some(), team.is_some());

            Some(LiveboardDto {
                team_id: player.team_id(),
                player_id: player.player_id,
                score: player.score,
            })
        }));
        liveboard.sort_by(|a, b| b.cmp(a));
        liveboard
    }

    /// Gets the leaders in the "current" liveboard without recalculation (or diffing).
    pub fn get(&self) -> &Arc<[LiveboardDto]> {
        &self.leaders
    }

    /// Gets initializer for new client.
    pub fn initializer(&self, player_id: PlayerId) -> LiveboardUpdate {
        LiveboardUpdate::Updated {
            added: Arc::clone(&self.leaders),
            removed: Vec::new().into(),
            your_score: self.rankings.get(&player_id).cloned(),
        }
    }

    /// Recalculates liveboard and generates a diff.
    #[allow(clippy::type_complexity)]
    pub fn delta(
        &mut self,
        players: &PlayerRepo<G>,
        #[cfg(feature = "teams")] teams: &TeamRepo<G>,
    ) -> Option<LiveboardDelta> {
        if self.update_rate_limiter.should_limit_rate() {
            return None;
        }

        let all_players_in_rank_order = Self::compute(
            players,
            #[cfg(feature = "teams")]
            teams,
        );

        let mut rank_updates = HashMap::new();
        for (ranking, (a, b)) in all_players_in_rank_order
            .iter()
            .zip(
                self.previous
                    .iter()
                    .map(Some)
                    .chain(std::iter::repeat(None)),
            )
            .enumerate()
        {
            if b.map(|b| a.player_id != b.player_id || a.score != b.score)
                .unwrap_or(true)
            {
                rank_updates.insert(
                    a.player_id,
                    YourScoreDto {
                        ranking: ranking.try_into().unwrap_or(0),
                        score: a.score,
                    },
                );
            }
        }
        let current_rankings = Arc::new(rank_updates);

        let current_liveboard: Vec<_> = all_players_in_rank_order
            .iter()
            .cloned()
            .take(G::LEADERBOARD_SIZE)
            .collect();

        let (added, removed) = diff_small_n(&self.leaders, &current_liveboard, |dto| dto.player_id)
            .map(|ar| (ar.0, ar.1))
            .unwrap_or((Vec::new(), Vec::new()));
        let liveboard_delta =
            LiveboardDelta(added.into(), removed.into(), Arc::clone(&current_rankings));
        self.leaders = current_liveboard.into();
        self.previous = all_players_in_rank_order;
        self.rankings = current_rankings;
        Some(liveboard_delta)
    }
}
