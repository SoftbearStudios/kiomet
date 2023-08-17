// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bot::BotRepo;
use crate::client::ClientRepo;
use crate::context::Context;
use crate::game_service::GameArenaService;
use crate::invitation::InvitationRepo;
use crate::leaderboard::LeaderboardRepo;
use crate::metric::MetricRepo;
use crate::plasma::PlasmaClient;
use core_protocol::dto::ServerDto;
use core_protocol::id::ServerId;
use core_protocol::ServerNumber;
use std::sync::Arc;

/// Contains a [`GameArenaService`] and the corresponding [`Context`].
pub struct ContextService<G: GameArenaService> {
    pub context: Context<G>,
    pub service: G,
}

impl<G: GameArenaService> ContextService<G> {
    pub fn new(
        min_bots: Option<usize>,
        max_bots: Option<usize>,
        bot_percent: Option<usize>,
        chat_log: Option<String>,
    ) -> Self {
        let bots = BotRepo::new_from_options(min_bots, max_bots, bot_percent);

        Self {
            service: G::new(bots.min_bots),
            context: Context::new(bots, chat_log),
        }
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn update(
        &mut self,
        clients: &mut ClientRepo<G>,
        leaderboard: &mut LeaderboardRepo<G>,
        invitations: &mut InvitationRepo<G>,
        metrics: &mut MetricRepo<G>,
        server_delta: &Option<(Arc<[ServerDto]>, Arc<[ServerNumber]>)>,
        server_id: ServerId,
        plasma: &PlasmaClient,
    ) {
        // Spawn/de-spawn clients and bots.
        clients.prune(
            &mut self.service,
            &mut self.context.players,
            #[cfg(feature = "teams")]
            &mut self.context.teams,
            invitations,
            metrics,
            server_id,
            self.context.token,
            plasma,
        );
        self.context
            .bots
            .update_count(&mut self.service, &mut self.context.players);

        // Update game logic.
        self.service.tick(&mut self.context);
        self.context.players.update_is_alive_and_team_id(
            &mut self.service,
            #[cfg(feature = "teams")]
            &mut self.context.teams,
            metrics,
        );

        // Update clients and bots.
        clients.update(
            &self.service,
            &mut self.context.players,
            #[cfg(feature = "teams")]
            &mut self.context.teams,
            &mut self.context.liveboard,
            leaderboard,
            server_delta,
        );
        self.context
            .bots
            .update(&self.service, &self.context.players);

        leaderboard.process(&self.context.liveboard, &self.context.players);

        // Post-update game logic.
        self.service.post_update(&mut self.context);

        // Bot commands/joining/leaving, postponed because no commands should be issued between
        // `GameService::tick` and `GameService::post_update`.
        self.context
            .bots
            .post_update(&mut self.service, &self.context.players);
    }
}
