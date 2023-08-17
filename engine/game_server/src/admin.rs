// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::client::ClientRepo;
use crate::context::Context;
use crate::game_service::GameArenaService;
use crate::infrastructure::Infrastructure;
use crate::metric::{Bundle, MetricBundle, MetricRepo};
use crate::player::PlayerRepo;
use crate::static_files::static_size_and_hash;
use actix::{fut, ActorFutureExt, Handler, ResponseActFuture, WrapFuture};
use core_protocol::dto::{AdminPlayerDto, MessageDto, SnippetDto};
use core_protocol::id::{PlayerId, RegionId, UserAgentId};
use core_protocol::metrics::{MetricFilter, Metrics};
use core_protocol::name::{PlayerAlias, Referrer};
use core_protocol::rpc::{AdminRequest, AdminUpdate};
use core_protocol::{get_unix_time_now, ClientHash, SnippetId};
use minicdn::{EmbeddedMiniCdn, MiniCdn};
use std::collections::HashMap;
use std::hash::Hash;
use std::iter;
use std::marker::PhantomData;
use std::net::{IpAddr, Ipv4Addr};
use std::str::FromStr;
use std::sync::{Arc, RwLock};
use std::time::Duration;

/// Responsible for the admin interface.
pub struct AdminRepo<G: GameArenaService> {
    game_client: Arc<RwLock<MiniCdn>>,
    pub(crate) client_hash: ClientHash,
    #[cfg(unix)]
    profile: Option<pprof::ProfilerGuard<'static>>,
    _spooky: PhantomData<G>,
}

impl<G: GameArenaService> AdminRepo<G> {
    pub fn new(game_client: Arc<RwLock<MiniCdn>>, client_hash: ClientHash) -> Self {
        Self {
            game_client,
            client_hash,
            #[cfg(unix)]
            profile: None,
            _spooky: PhantomData,
        }
    }

    /// Get list of games hosted by the server.
    fn request_games(&self) -> Result<AdminUpdate, &'static str> {
        // We only support one game type per server.
        Ok(AdminUpdate::GamesRequested(
            vec![(G::GAME_ID, 1.0)].into_boxed_slice(),
        ))
    }

    /// Get admin view of real players in the game.
    fn request_players(&self, players: &PlayerRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::PlayersRequested(
            players
                .iter_borrow()
                .filter_map(|player| {
                    if let Some(client) = player.client().filter(|_| !player.is_out_of_game()) {
                        Some(AdminPlayerDto {
                            alias: client.alias,
                            player_id: player.player_id,
                            team_id: player.team_id(),
                            region_id: client.metrics.region_id,
                            session_token: client.session_token,
                            ip_address: client.ip_address,
                            moderator: client.moderator,
                            score: player.score,
                            plays: client.metrics.plays,
                            fps: client.metrics.fps,
                            rtt: client.metrics.rtt,
                            messages: client.chat.context.total(),
                            inappropriate_messages: client.chat.context.total_inappropriate(),
                            abuse_reports: client.chat.context.reports(),
                            mute: seconds_ceil(client.chat.context.muted_for()),
                            restriction: seconds_ceil(client.chat.context.restricted_for()),
                        })
                    } else {
                        None
                    }
                })
                .collect(),
        ))
    }

    /// (Temporarily) overrides the alias of a given real player.
    fn override_player_alias(
        &self,
        player_id: PlayerId,
        alias: PlayerAlias,
        players: &PlayerRepo<G>,
    ) -> Result<AdminUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("nonexistent player")?;
        let client = player.client_mut().ok_or("not a real player")?;
        // We still censor, in case of unauthorized admin access.
        let censored = PlayerAlias::new_sanitized(alias.as_str());
        client.alias = censored;
        Ok(AdminUpdate::PlayerAliasOverridden(censored))
    }

    /// (Temporarily) overrides the moderator status of a given real player.
    fn override_player_moderator(
        &self,
        player_id: PlayerId,
        moderator: bool,
        players: &PlayerRepo<G>,
    ) -> Result<AdminUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("nonexistent player")?;
        let client = player.client_mut().ok_or("not a real player")?;
        client.moderator = moderator;
        Ok(AdminUpdate::PlayerModeratorOverridden(moderator))
    }

    /// Mutes a given real player for a configurable amount of minutes (0 means disable mute).
    fn mute_player(
        &self,
        player_id: PlayerId,
        minutes: usize,
        players: &PlayerRepo<G>,
    ) -> Result<AdminUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("nonexistent player")?;
        let client = player.client_mut().ok_or("not a real player")?;
        client
            .chat
            .context
            .mute_for(Duration::from_secs(minutes as u64 * 60));
        Ok(AdminUpdate::PlayerMuted(seconds_ceil(
            client.chat.context.muted_for(),
        )))
    }

    /// Restrict a given real player's chat to safe phrases for a configurable amount of minutes
    /// (0 means disable restriction).
    fn restrict_player(
        &self,
        player_id: PlayerId,
        minutes: usize,
        players: &PlayerRepo<G>,
    ) -> Result<AdminUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("nonexistent player")?;
        let client = player.client_mut().ok_or("not a real player")?;
        client
            .chat
            .context
            .restrict_for(Duration::from_secs(minutes as u64 * 60));
        Ok(AdminUpdate::PlayerRestricted(seconds_ceil(
            client.chat.context.restricted_for(),
        )))
    }

    fn request_snippets(clients: &ClientRepo<G>) -> Result<AdminUpdate, &'static str> {
        let mut list: Vec<SnippetDto> = clients
            .snippets
            .iter()
            .map(|(snippet_id, snippet)| SnippetDto {
                snippet_id: *snippet_id,
                snippet: Arc::clone(snippet),
            })
            .collect();
        list.sort();
        Ok(AdminUpdate::SnippetsRequested(list.into()))
    }

    fn clear_snippet(
        clients: &mut ClientRepo<G>,
        snippet_id: SnippetId,
    ) -> Result<AdminUpdate, &'static str> {
        if clients.snippets.remove(&snippet_id).is_some() {
            Ok(AdminUpdate::SnippetCleared)
        } else {
            Err("snippet not found")
        }
    }

    fn set_snippet(
        clients: &mut ClientRepo<G>,
        snippet_id: SnippetId,
        snippet: Arc<str>,
    ) -> Result<AdminUpdate, &'static str> {
        if snippet.len() > 4096 {
            Err("snippet too long")
        } else {
            clients.snippets.insert(snippet_id, snippet);
            Ok(AdminUpdate::SnippetSet)
        }
    }

    /// Request summary of metrics for the current calendar calendar hour.
    fn request_summary(
        infrastructure: &mut Infrastructure<G>,
        filter: Option<MetricFilter>,
    ) -> Result<AdminUpdate, &'static str> {
        let current = MetricRepo::get_metrics(infrastructure, filter);

        // One hour.
        // MetricRepo::get_metrics(infrastructure, filter).summarize(),
        let mut summary = infrastructure
            .metrics
            .history
            .oldest_ordered()
            .map(|bundle: &MetricBundle| bundle.metric(filter))
            .chain(iter::once(current.clone()))
            .sum::<Metrics>()
            .summarize();

        // TODO: Make special [`DiscreteMetric`] that handles data that is not necessarily unique.
        summary.arenas_cached.total = current.arenas_cached.total;
        summary.invitations_cached.total = current.invitations_cached.total;
        summary.players_cached.total = current.players_cached.total;
        summary.sessions_cached.total = current.sessions_cached.total;

        Ok(AdminUpdate::SummaryRequested(Box::new(summary)))
    }

    /// Request metric data points for the last 24 calendar hours (excluding the current hour, in
    /// which metrics are incomplete).
    fn request_day(
        metrics: &MetricRepo<G>,
        filter: Option<MetricFilter>,
    ) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::DayRequested(
            metrics
                .history
                .oldest_ordered()
                .map(|bundle| (bundle.start, bundle.data_point(filter)))
                .collect(),
        ))
    }

    fn request_category_inner<T: Hash + Eq + Copy>(
        &self,
        initial: impl IntoIterator<Item = T>,
        extract: impl Fn(&Bundle<Metrics>) -> &HashMap<T, Metrics>,
        metrics: &MetricRepo<G>,
    ) -> Box<[(T, f32)]> {
        let initial = initial.into_iter();
        let mut hash: HashMap<T, u32> = HashMap::with_capacity(initial.size_hint().0);
        for tracked in initial {
            hash.insert(tracked, 0);
        }
        let mut total = 0u32;
        for bundle in iter::once(&metrics.current).chain(metrics.history.iter()) {
            for (&key, metrics) in extract(&bundle.bundle).iter() {
                *hash.entry(key).or_default() += metrics.visits.total;
            }
            total += bundle.bundle.total.visits.total;
        }
        let mut list: Vec<(T, u32)> = hash.into_iter().collect();
        // Sort in reverse so higher counts are first.
        list.sort_unstable_by_key(|(_, count)| u32::MAX - count);
        let mut percents: Vec<_> = list
            .into_iter()
            .map(|(v, count)| (v, count as f32 / total as f32))
            .collect();
        percents.truncate(20);
        percents.into_boxed_slice()
    }

    /// Request a list of referrers, sorted by percentage, and truncated to a reasonable limit.
    fn request_referrers(&self, metrics: &MetricRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::ReferrersRequested(
            self.request_category_inner(
                Referrer::TRACKED.map(|s| Referrer::from_str(s).unwrap()),
                |bundle| &bundle.by_referrer,
                metrics,
            ),
        ))
    }

    /// Request a list of user agents, sorted by percentage.
    fn request_user_agents(&self, metrics: &MetricRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::UserAgentsRequested(
            self.request_category_inner(
                UserAgentId::iter(),
                |bundle| &bundle.by_user_agent_id,
                metrics,
            ),
        ))
    }

    /// Request a list of regions, sorted by percentage.
    fn request_regions(&self, metrics: &MetricRepo<G>) -> Result<AdminUpdate, &'static str> {
        Ok(AdminUpdate::RegionsRequested(self.request_category_inner(
            RegionId::iter(),
            |bundle| &bundle.by_region_id,
            metrics,
        )))
    }

    /// Send a chat to all players on the server, or a specific player (in which case, will send a
    /// whisper message).
    fn send_chat(
        &self,
        player_id: Option<PlayerId>,
        alias: PlayerAlias,
        message: String,
        context: &mut Context<G>,
    ) -> Result<AdminUpdate, &'static str> {
        context.chat.log_chat(
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            alias,
            &message,
            false,
            "ok",
        );

        let message = MessageDto {
            alias,
            date_sent: get_unix_time_now(),
            player_id: None,
            user_id: None,
            team_captain: false,
            team_name: None,
            text: message,
            authentic: true,
            whisper: player_id.is_some(),
        };

        if let Some(player_id) = player_id {
            let mut player = context
                .players
                .borrow_player_mut(player_id)
                .ok_or("nonexistent player")?;
            let client = player.client_mut().ok_or("not a real player")?;
            client.chat.receive(&Arc::new(message));
        } else {
            context
                .chat
                .broadcast_message(Arc::new(message), &mut context.players);
        }

        Ok(AdminUpdate::ChatSent)
    }

    fn set_game_client(
        &mut self,
        game_client: EmbeddedMiniCdn,
    ) -> Result<AdminUpdate, &'static str> {
        if game_client.get("index.html").is_none() {
            Err("no index.html")
        } else {
            let cdn = MiniCdn::Embedded(game_client);
            self.client_hash = static_size_and_hash(&cdn).1;
            *self.game_client.write().unwrap() = cdn;
            Ok(AdminUpdate::GameClientSet(self.client_hash))
        }
    }

    fn set_rustrict_trie(&mut self, trie: rustrict::Trie) -> Result<AdminUpdate, &'static str> {
        unsafe {
            *rustrict::Trie::customize_default() = trie;
        }
        Ok(AdminUpdate::RustrictTrieSet)
    }

    fn set_rustrict_replacements(
        &mut self,
        replacements: rustrict::Replacements,
    ) -> Result<AdminUpdate, &'static str> {
        unsafe {
            *rustrict::Replacements::customize_default() = replacements;
        }
        Ok(AdminUpdate::RustrictReplacementsSet)
    }

    fn start_profile(&mut self) -> Result<(), &'static str> {
        #[cfg(not(unix))]
        return Err("profile only available on Unix");

        #[cfg(unix)]
        if self.profile.is_some() {
            Err("profile already started")
        } else {
            self.profile =
                Some(pprof::ProfilerGuard::new(1000).map_err(|_| "failed to start profile")?);
            Ok(())
        }
    }

    fn finish_profile(&mut self) -> Result<AdminUpdate, &'static str> {
        #[cfg(not(unix))]
        return Err("profile only available on Unix");

        #[cfg(unix)]
        if let Some(profile) = self.profile.as_mut() {
            if let Ok(report) = profile.report().build() {
                self.profile = None;

                let mut buf = Vec::new();
                report
                    .flamegraph(&mut buf)
                    .map_err(|_| "error writing profiler flamegraph")?;

                Ok(AdminUpdate::ProfileRequested(
                    String::from_utf8(buf).map_err(|_| "profile contained invalid utf8")?,
                ))
            } else {
                Err("error building profile report")
            }
        } else {
            Err("profile not started or was interrupted")
        }
    }
}

impl<G: GameArenaService> Handler<AdminRequest> for Infrastructure<G> {
    type Result = ResponseActFuture<Self, Result<AdminUpdate, &'static str>>;

    fn handle(&mut self, request: AdminRequest, _ctx: &mut Self::Context) -> Self::Result {
        match request {
            AdminRequest::RequestSnippets => {
                Box::pin(fut::ready(AdminRepo::request_snippets(&self.clients)))
            }
            AdminRequest::ClearSnippet { snippet_id } => Box::pin(fut::ready(
                AdminRepo::clear_snippet(&mut self.clients, snippet_id),
            )),
            AdminRequest::SetSnippet {
                snippet_id,
                snippet,
            } => Box::pin(fut::ready(AdminRepo::set_snippet(
                &mut self.clients,
                snippet_id,
                snippet,
            ))),
            // Handle asynchronous requests (i.e. those that access database).
            AdminRequest::RequestSeries { .. } => {
                Box::pin(Box::pin(fut::ready(Err("failed to load"))))
            }
            AdminRequest::RequestDay { filter } => {
                Box::pin(fut::ready(AdminRepo::request_day(&self.metrics, filter)))
            }
            AdminRequest::RequestGames => Box::pin(fut::ready(self.admin.request_games())),
            AdminRequest::RequestPlayers => Box::pin(fut::ready(
                self.admin
                    .request_players(&self.arenas.main().context.players),
            )),
            AdminRequest::OverridePlayerAlias { player_id, alias } => {
                Box::pin(fut::ready(self.admin.override_player_alias(
                    player_id,
                    alias,
                    &self.arenas.main().context.players,
                )))
            }
            AdminRequest::OverridePlayerModerator {
                player_id,
                moderator,
            } => Box::pin(fut::ready(self.admin.override_player_moderator(
                player_id,
                moderator,
                &self.arenas.main().context.players,
            ))),
            AdminRequest::RestrictPlayer { player_id, minutes } => Box::pin(fut::ready(
                self.admin
                    .restrict_player(player_id, minutes, &self.arenas.main().context.players),
            )),
            AdminRequest::MutePlayer { player_id, minutes } => Box::pin(fut::ready(
                self.admin
                    .mute_player(player_id, minutes, &self.arenas.main().context.players),
            )),
            AdminRequest::RequestServerId => Box::pin(fut::ready(Ok(
                AdminUpdate::ServerIdRequested(self.server_id),
            ))),
            AdminRequest::RequestSummary { filter } => {
                Box::pin(fut::ready(AdminRepo::request_summary(self, filter)))
            }
            AdminRequest::RequestReferrers => {
                Box::pin(fut::ready(self.admin.request_referrers(&self.metrics)))
            }
            AdminRequest::RequestRegions => {
                Box::pin(fut::ready(self.admin.request_regions(&self.metrics)))
            }
            AdminRequest::RequestUserAgents => {
                Box::pin(fut::ready(self.admin.request_user_agents(&self.metrics)))
            }
            AdminRequest::SendChat {
                player_id,
                alias,
                message,
            } => Box::pin(fut::ready(self.admin.send_chat(
                player_id,
                alias,
                message,
                &mut self.arenas.main_mut().context,
            ))),
            AdminRequest::SetGameClient(client) => {
                Box::pin(fut::ready(self.admin.set_game_client(client)))
            }
            AdminRequest::SetRustrictTrie(trie) => {
                Box::pin(fut::ready(self.admin.set_rustrict_trie(trie)))
            }
            AdminRequest::SetRustrictReplacements(replacements) => Box::pin(fut::ready(
                self.admin.set_rustrict_replacements(replacements),
            )),
            AdminRequest::RequestProfile => {
                if let Err(e) = self.admin.start_profile() {
                    Box::pin(fut::ready(Err(e)))
                } else {
                    Box::pin(
                        tokio::time::sleep(Duration::from_secs(10))
                            .into_actor(self)
                            .map(move |_res, act, _ctx| act.admin.finish_profile()),
                    )
                }
            }
        }
    }
}

/// Converts a duration to seconds, rounding up.
fn seconds_ceil(duration: Duration) -> usize {
    ((duration.as_secs() + 59) / 60) as usize
}
