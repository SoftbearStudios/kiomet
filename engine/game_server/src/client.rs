// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::chat::{ChatRepo, ClientChatData};
use crate::game_service::GameArenaService;
use crate::infrastructure::Infrastructure;
use crate::invitation::{ClientInvitationData, InvitationRepo};
use crate::leaderboard::LeaderboardRepo;
use crate::liveboard::LiveboardRepo;
use crate::metric::{ClientMetricData, MetricRepo};
use crate::plasma::PlasmaClient;
use crate::player::{PlayerData, PlayerRepo, PlayerTuple};
use crate::system::SystemRepo;
use actix::{Context as ActorContext, Handler, Message};
use atomic_refcell::AtomicRefCell;
use core_protocol::dto::{InvitationDto, ServerDto};
use core_protocol::id::{CohortId, InvitationId, PlayerId, ServerId, UserAgentId};
use core_protocol::name::{PlayerAlias, Referrer};
use core_protocol::rpc::{
    AdType, ClientRequest, ClientUpdate, LeaderboardUpdate, LiveboardUpdate, PlayerUpdate, Request,
    SystemUpdate, Update,
};
use core_protocol::{
    get_unix_time_now, ArenaToken, NickName, PlasmaRequestV1, RealmName, ServerNumber,
    SessionToken, SnippetId, Token, UnixTime, UserId,
};
use log::{error, info, warn};
use maybe_parallel_iterator::IntoMaybeParallelRefIterator;
use rand::{thread_rng, Rng};
use rust_embed::RustEmbed;
use server_util::generate_id::generate_id;
use server_util::ip_rate_limiter::IpRateLimiter;
use server_util::observer::{ObserverMessage, ObserverMessageBody, ObserverUpdate};
use server_util::rate_limiter::{RateLimiter, RateLimiterProps};
use std::borrow::Cow;
use std::collections::hash_map::Entry;
use std::collections::{HashMap, HashSet};
use std::fs::OpenOptions;
use std::io::Write;
use std::marker::PhantomData;
use std::net::IpAddr;
use std::str::{self, FromStr};
use std::sync::Arc;
use std::time::{Duration, Instant};
use tokio::sync::mpsc::UnboundedSender;

#[cfg(feature = "teams")]
use crate::team::{ClientTeamData, TeamRepo};
#[cfg(feature = "teams")]
use core_protocol::rpc::TeamUpdate;
#[cfg(feature = "teams")]
use std::ops::Deref;

/// Directed to a websocket future corresponding to a client.
pub type ClientAddr<G> =
    UnboundedSender<ObserverUpdate<Update<<G as GameArenaService>::GameUpdate>>>;

/// Keeps track of clients a.k.a. real players a.k.a. websockets.
pub struct ClientRepo<G: GameArenaService> {
    authenticate_rate_limiter: IpRateLimiter,
    prune_rate_limiter: RateLimiter,
    pub(crate) snippets: HashMap<SnippetId, Arc<str>>,
    /// Where to log traces to.
    trace_log: Option<Arc<str>>,
    _spooky: PhantomData<G>,
}

#[derive(RustEmbed)]
#[folder = "./src/snippets"]
struct ReferrerSnippet;

impl<G: GameArenaService> ClientRepo<G> {
    pub fn new(trace_log: Option<String>, authenticate: RateLimiterProps) -> Self {
        Self {
            authenticate_rate_limiter: authenticate.into(),
            prune_rate_limiter: RateLimiter::new(Duration::from_secs(1), 0),
            snippets: Self::load_default_snippets(),
            trace_log: trace_log.map(Into::into),
            _spooky: PhantomData,
        }
    }

    fn load_default_snippets() -> HashMap<SnippetId, Arc<str>> {
        let mut hash_map = HashMap::new();
        for key in ReferrerSnippet::iter() {
            let value = ReferrerSnippet::get(&key).map(|f| f.data);
            if let Some(value) = value {
                let Ok(snippet_id) = SnippetId::from_str(&key).map_err(|e| {
                    error!("invalid snippet_id {key:?}: {e}");
                }) else {
                    continue;
                };

                match str::from_utf8(&value) {
                    Ok(js_src) => match hash_map.entry(snippet_id) {
                        Entry::Vacant(e) => {
                            info!("loaded snippet {snippet_id}");
                            e.insert(js_src.into());
                        }
                        Entry::Occupied(_) => error!("duplicate snippet {snippet_id}"),
                    },
                    Err(e) => {
                        error!("invalid UTF-8 in snippet: {:?}", e);
                    }
                }
            }
        }
        hash_map
    }

    /// Client websocket (re)connected.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn register(
        &mut self,
        player_id: PlayerId,
        register_observer: ClientAddr<G>,
        players: &mut PlayerRepo<G>,
        #[cfg(feature = "teams")] teams: &mut TeamRepo<G>,
        chat: &ChatRepo<G>,
        leaderboards: &LeaderboardRepo<G>,
        liveboard: &LiveboardRepo<G>,
        metrics: &mut MetricRepo<G>,
        system: &SystemRepo<G>,
        server_id: ServerId,
        realm_name: Option<RealmName>,
        game: &mut G,
    ) {
        let player_tuple = match players.get(player_id) {
            Some(player_tuple) => player_tuple,
            None => {
                debug_assert!(false, "client gone in register");
                return;
            }
        };

        let mut player = player_tuple.borrow_player_mut();

        let client = match player.client_mut() {
            Some(client) => client,
            None => {
                debug_assert!(false, "register wasn't a client");
                return;
            }
        };

        // Welcome the client in.
        let _ = register_observer.send(ObserverUpdate::Send {
            message: Update::Client(ClientUpdate::SessionCreated {
                cohort_id: client.metrics.cohort_id,
                server_number: server_id.cloud_server_number(),
                realm_name,
                player_id,
                token: client.token,
                date_created: client.metrics.date_created,
            }),
        });

        // Don't assume client remembered anything, although it may/should have.
        *client.data.borrow_mut() = G::ClientData::default();
        client.chat.forget_state();
        #[cfg(feature = "teams")]
        client.team.forget_state();

        // If there is a JS snippet for the cohort and referrer, send it to client for eval.
        let snippet = client
            .metrics
            .referrer
            .and_then(|referrer| {
                self.snippets.get(&SnippetId {
                    cohort_id: Some(client.metrics.cohort_id),
                    referrer: Some(referrer),
                })
            })
            .or_else(|| {
                client.metrics.referrer.and_then(|referrer| {
                    self.snippets.get(&SnippetId {
                        cohort_id: None,
                        referrer: Some(referrer),
                    })
                })
            })
            .or_else(|| {
                self.snippets.get(&SnippetId {
                    cohort_id: Some(client.metrics.cohort_id),
                    referrer: None,
                })
            })
            .or_else(|| self.snippets.get(&Default::default()));

        if let Some(snippet) = snippet {
            let _ = register_observer.send(ObserverUpdate::Send {
                message: Update::Client(ClientUpdate::EvalSnippet(snippet.clone())),
            });
        }

        // Change status to connected.
        let new_status = ClientStatus::Connected {
            observer: register_observer.clone(),
        };
        let old_status = std::mem::replace(&mut client.status, new_status);

        match old_status {
            ClientStatus::Connected { observer } => {
                // If it still exists, old client is now retired.
                let _ = observer.send(ObserverUpdate::Close);
                drop(player);
            }
            ClientStatus::Limbo { .. } => {
                info!("player {:?} restored from limbo", player_id);
                drop(player);
            }
            ClientStatus::Pending { .. } => {
                metrics.start_visit(client);

                drop(player);

                // We previously left the game, so now we have to rejoin.
                game.player_joined(player_tuple, &*players);
            }
            ClientStatus::LeavingLimbo { .. } => {
                drop(player);

                // We previously left the game, so now we have to rejoin.
                game.player_joined(player_tuple, &*players);
            }
        }

        // Send initial data.
        for initializer in leaderboards.initializers() {
            let _ = register_observer.send(ObserverUpdate::Send {
                message: Update::Leaderboard(initializer),
            });
        }

        let _ = register_observer.send(ObserverUpdate::Send {
            message: Update::Liveboard(liveboard.initializer(player_id)),
        });

        let chat_success = chat.initialize_client(player_id, players);
        debug_assert!(chat_success);

        let _ = register_observer.send(ObserverUpdate::Send {
            message: Update::Player(players.initializer()),
        });

        #[cfg(feature = "teams")]
        if let Some(initializer) = teams.initializer() {
            let _ = register_observer.send(ObserverUpdate::Send {
                message: Update::Team(initializer),
            });
        }

        if let Some(initializer) = system.initializer() {
            let _ = register_observer.send(ObserverUpdate::Send {
                message: Update::System(initializer),
            });
        }
    }

    /// Client websocket disconnected.
    pub(crate) fn unregister(
        &mut self,
        player_id: PlayerId,
        unregister_observer: ClientAddr<G>,
        players: &PlayerRepo<G>,
    ) {
        // There is a possible race condition to handle:
        //  1. Client A registers
        //  3. Client B registers with the same session and player so evicts client A from limbo
        //  2. Client A unregisters and is placed in limbo

        let mut player = match players.borrow_player_mut(player_id) {
            Some(player) => player,
            None => return,
        };

        let client = match player.client_mut() {
            Some(client) => client,
            None => return,
        };

        if let ClientStatus::Connected { observer } = &client.status {
            if observer.same_channel(&unregister_observer) {
                client.status = ClientStatus::Limbo {
                    expiry: Instant::now() + G::LIMBO,
                };
                info!("player {:?} is in limbo", player_id);
            }
        }
    }

    /// Update all clients with game state.
    #[allow(clippy::type_complexity)]
    pub(crate) fn update(
        &mut self,
        game: &G,
        players: &mut PlayerRepo<G>,
        #[cfg(feature = "teams")] teams: &mut TeamRepo<G>,
        liveboard: &mut LiveboardRepo<G>,
        leaderboard: &LeaderboardRepo<G>,
        server_delta: &Option<(Arc<[ServerDto]>, Arc<[ServerNumber]>)>,
    ) {
        let player_update = players.delta(
            #[cfg(feature = "teams")]
            &*teams,
        );
        #[cfg(feature = "teams")]
        let team_update = teams.delta(&*players);
        let immut_players = &*players;
        let player_chat_team_updates: HashMap<PlayerId, _> = players
            .iter_player_ids()
            .filter(|&id| {
                !id.is_bot()
                    && immut_players
                        .borrow_player(id)
                        .unwrap()
                        .client()
                        .map(|c| matches!(c.status, ClientStatus::Connected { .. }))
                        .unwrap_or(false)
            })
            .map(|player_id| {
                (
                    player_id,
                    (
                        ChatRepo::<G>::player_delta(player_id, immut_players),
                        #[cfg(feature = "teams")]
                        teams.player_delta(player_id, immut_players).unwrap(),
                    ),
                )
            })
            .collect();
        let liveboard_update = liveboard.delta(
            &*players,
            #[cfg(feature = "teams")]
            &*teams,
        );
        let leaderboard_update: Vec<_> = leaderboard.deltas_nondestructive().collect();

        let players = &*players;
        players.players.maybe_par_iter().for_each(
            move |(player_id, player_tuple): (&PlayerId, &Arc<PlayerTuple<G>>)| {
                let player = player_tuple.borrow_player();

                let client_data = match player.client() {
                    Some(client) => client,
                    None => return,
                };

                // In limbo or will be soon (not connected, cannot send an update).
                if let ClientStatus::Connected { observer } = &client_data.status {
                    if let Some(update) = game.get_game_update(
                        player_tuple,
                        &mut *client_data.data.borrow_mut(),
                        players,
                    ) {
                        let _ = observer.send(ObserverUpdate::Send {
                            message: Update::Game(update),
                        });
                    }

                    if let Some((added, removed, real_players)) = player_update.as_ref() {
                        let _ = observer.send(ObserverUpdate::Send {
                            message: Update::Player(PlayerUpdate::Updated {
                                added: Arc::clone(added),
                                removed: Arc::clone(removed),
                                real_players: *real_players,
                            }),
                        });
                    }

                    #[cfg(feature = "teams")]
                    if let Some((added, removed)) = team_update.as_ref() {
                        if !added.is_empty() {
                            let _ = observer.send(ObserverUpdate::Send {
                                message: Update::Team(TeamUpdate::AddedOrUpdated(Arc::clone(
                                    added,
                                ))),
                            });
                        }
                        if !removed.is_empty() {
                            let _ = observer.send(ObserverUpdate::Send {
                                message: Update::Team(TeamUpdate::Removed(Arc::clone(removed))),
                            });
                        }
                    }

                    if let Some(player_chat_team_update) = player_chat_team_updates.get(player_id) {
                        if let Some(chat_update) = &player_chat_team_update.0 {
                            let _ = observer.send(ObserverUpdate::Send {
                                message: Update::Chat(chat_update.clone()),
                            });
                        }

                        #[cfg(feature = "teams")]
                        {
                            let (members, joiners, joins) = &player_chat_team_update.1;
                            // TODO: We could get members on a per team basis.
                            if let Some(members) = members {
                                let _ = observer.send(ObserverUpdate::Send {
                                    message: Update::Team(TeamUpdate::Members(
                                        members.deref().clone().into(),
                                    )),
                                });
                            }

                            if let Some(joiners) = joiners {
                                let _ = observer.send(ObserverUpdate::Send {
                                    message: Update::Team(TeamUpdate::Joiners(
                                        joiners.deref().clone().into(),
                                    )),
                                });
                            }

                            if let Some(joins) = joins {
                                let _ = observer.send(ObserverUpdate::Send {
                                    message: Update::Team(TeamUpdate::Joins(
                                        joins.iter().cloned().collect(),
                                    )),
                                });
                            }
                        }
                    } else {
                        debug_assert!(
                            false,
                            "not possible, all connected clients should have an entry"
                        );
                    }

                    for &(period_id, leaderboard) in &leaderboard_update {
                        let _ = observer.send(ObserverUpdate::Send {
                            message: Update::Leaderboard(LeaderboardUpdate::Updated(
                                period_id,
                                Arc::clone(leaderboard),
                            )),
                        });
                    }

                    if let Some((added, removed, your_score)) =
                        liveboard_update.as_ref().and_then(|u| u.cloned(player_id))
                    {
                        let _ = observer.send(ObserverUpdate::Send {
                            message: Update::Liveboard(LiveboardUpdate::Updated {
                                added,
                                removed,
                                your_score,
                            }),
                        });
                    }

                    if let Some((added, removed)) = server_delta.as_ref() {
                        if !added.is_empty() {
                            let _ = observer.send(ObserverUpdate::Send {
                                message: Update::System(SystemUpdate::Added(Arc::clone(added))),
                            });
                        }
                        if !removed.is_empty() {
                            let _ = observer.send(ObserverUpdate::Send {
                                message: Update::System(SystemUpdate::Removed(Arc::clone(removed))),
                            });
                        }
                    }
                }
            },
        );
    }

    /// Cleans up old clients. Rate limited internally.
    #[allow(clippy::too_many_arguments)]
    pub(crate) fn prune(
        &mut self,
        service: &mut G,
        players: &mut PlayerRepo<G>,
        #[cfg(feature = "teams")] teams: &mut TeamRepo<G>,
        invitations: &mut InvitationRepo<G>,
        metrics: &mut MetricRepo<G>,
        server_id: ServerId,
        arena_token: ArenaToken,
        plasma: &PlasmaClient,
    ) {
        let now = Instant::now();

        if self.prune_rate_limiter.should_limit_rate_with_now(now) {
            return;
        }

        let immut_players = &*players;
        let to_forget: Vec<PlayerId> = immut_players
            .players
            .iter()
            .filter(|&(&player_id, player_tuple)| {
                let mut player = player_tuple.borrow_player_mut();
                let was_alive = player.was_alive;
                if let Some(client_data) = player.client_mut() {
                    match &client_data.status {
                        ClientStatus::Connected { .. } => {
                            // Wait for transition to limbo via unregister, which is the "proper" channel.
                            false
                        }
                        ClientStatus::Limbo { expiry } => {
                            if &now >= expiry {
                                client_data.status = ClientStatus::LeavingLimbo { since: now };
                                drop(player);
                                service.player_left(player_tuple, immut_players);
                            }
                            false
                        }
                        ClientStatus::LeavingLimbo { since } => {
                            if was_alive {
                                debug_assert!(
                                    since.elapsed() < Duration::from_secs(1),
                                    "player left game but still alive"
                                );
                                false
                            } else {
                                if let Some((user_id, session_token)) =
                                    client_data.user_id.zip(client_data.session_token)
                                {
                                    plasma.do_request(PlasmaRequestV1::UnregisterPlayer {
                                        game_id: G::GAME_ID,
                                        server_id,
                                        user_id,
                                        realm_name: None,
                                        arena_token,
                                        session_token,
                                    });
                                }
                                metrics.stop_visit(&mut *player);
                                info!("player_id {:?} expired from limbo", player_id);

                                true
                            }
                        }
                        ClientStatus::Pending { expiry } => {
                            // Not actually in game, so no cleanup required.
                            &now > expiry
                        }
                    }
                } else {
                    false
                }
            })
            .map(|(&player_id, _)| player_id)
            .collect();

        for player_id in to_forget {
            players.forget(
                player_id,
                #[cfg(feature = "teams")]
                teams,
                invitations,
            );
        }
    }

    /// Handles [`G::Command`]'s.
    fn handle_game_command(
        player_id: PlayerId,
        command: G::GameRequest,
        service: &mut G,
        players: &PlayerRepo<G>,
    ) -> Result<Option<G::GameUpdate>, &'static str> {
        if let Some(player_data) = players.get(player_id) {
            // Game updates for all players are usually processed at once, but we also allow
            // one-off responses.
            Ok(service.player_command(command, player_data, players))
        } else {
            Err("nonexistent observer")
        }
    }

    fn login(
        players: &PlayerRepo<G>,
        server_id: ServerId,
        arena_token: ArenaToken,
        player_id: PlayerId,
        session_token: SessionToken,
        plasma: &PlasmaClient,
    ) -> Result<ClientUpdate, &'static str> {
        if let Some(mut player) = players.borrow_player_mut(player_id) {
            if let Some(client) = player.client_mut() {
                if client.session_token != Some(session_token) {
                    client.session_token = Some(session_token);
                    client.user_id = None;
                    client.nick_name = None;
                    client.moderator = false;
                    plasma.do_request(PlasmaRequestV1::RegisterPlayer {
                        game_id: G::GAME_ID,
                        server_id,
                        player_id,
                        arena_token,
                        session_token,
                        realm_name: None,
                    });
                }
                Ok(ClientUpdate::LoggedIn(session_token))
            } else {
                debug_assert!(false);
                Err("bot")
            }
        } else {
            Err("nonexistent observer")
        }
    }

    /// Request a different alias (may not be done while alive).
    fn set_alias(
        player_id: PlayerId,
        alias: PlayerAlias,
        players: &PlayerRepo<G>,
    ) -> Result<ClientUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("player doesn't exist")?;

        if player
            .alive_duration()
            .map(|d| d > Duration::from_secs(1))
            .unwrap_or(false)
        {
            return Err("cannot change alias while alive");
        }

        let client = player.client_mut().ok_or("only clients can set alias")?;
        let censored_alias = PlayerAlias::new_sanitized(alias.as_str());
        client.alias = censored_alias;
        Ok(ClientUpdate::AliasSet(censored_alias))
    }

    /// Record client frames per second (FPS) for statistical purposes.
    fn tally_ad(
        player_id: PlayerId,
        ad_type: AdType,
        players: &PlayerRepo<G>,
        metrics: &mut MetricRepo<G>,
    ) -> Result<ClientUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("player doesn't exist")?;
        let client = player.client_mut().ok_or("only clients can tally ads")?;
        metrics.mutate_with(
            |metrics| {
                let metric = match ad_type {
                    AdType::Banner => &mut metrics.banner_ads,
                    AdType::Rewarded => &mut metrics.rewarded_ads,
                    AdType::Video => &mut metrics.video_ads,
                };
                metric.increment();
            },
            &client.metrics,
        );
        Ok(ClientUpdate::AdTallied)
    }

    /// Record client frames per second (FPS) for statistical purposes.
    fn tally_fps(
        player_id: PlayerId,
        fps: f32,
        players: &PlayerRepo<G>,
    ) -> Result<ClientUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("player doesn't exist")?;
        let client = player.client_mut().ok_or("only clients can tally fps")?;

        client.metrics.fps = sanitize_tps(fps);
        if client.metrics.fps.is_some() {
            Ok(ClientUpdate::FpsTallied)
        } else {
            Err("invalid fps")
        }
    }

    /// Record a client-side error message for investigation.
    fn trace(
        &self,
        player_id: PlayerId,
        message: String,
        players: &PlayerRepo<G>,
        metrics: &mut MetricRepo<G>,
    ) -> Result<ClientUpdate, &'static str> {
        let mut player = players
            .borrow_player_mut(player_id)
            .ok_or("player doesn't exist")?;
        let client = player.client_mut().ok_or("only clients can trace")?;

        #[cfg(debug_assertions)]
        let trace_limit = None;
        #[cfg(not(debug_assertions))]
        let trace_limit = Some(25);

        if message.len() > 4096 {
            Err("trace too long")
        } else if trace_limit
            .map(|limit| client.traces < limit)
            .unwrap_or(true)
        {
            metrics.mutate_with(
                |metrics| {
                    metrics.crashes.increment();
                },
                &client.metrics,
            );
            if let Some(trace_log) = self.trace_log.as_ref() {
                let trace_log = Arc::clone(trace_log);
                let mut line = Vec::with_capacity(256);
                let mut writer = csv::Writer::from_writer(&mut line);
                if let Err(e) = writer.write_record(
                    [
                        get_unix_time_now().to_string().as_str(),
                        &format!("{:?}", G::GAME_ID),
                        &client.ip_address.to_string(),
                        &client
                            .metrics
                            .region_id
                            .map(|r| Cow::Owned(format!("{:?}", r)))
                            .unwrap_or(Cow::Borrowed("?")),
                        client
                            .metrics
                            .referrer
                            .as_ref()
                            .map(|r| r.as_str())
                            .unwrap_or("?"),
                        &client
                            .metrics
                            .user_agent_id
                            .map(|ua| Cow::Owned(format!("{:?}", ua)))
                            .unwrap_or(Cow::Borrowed("?")),
                        &message,
                    ]
                    .as_slice(),
                ) {
                    error!("error composing trace line: {:?}", e);
                } else {
                    drop(writer);
                    tokio::task::spawn_blocking(move || {
                        if let Err(e) = OpenOptions::new()
                            .create(true)
                            .append(true)
                            .open(&*trace_log)
                            .and_then(move |mut file| file.write_all(&line))
                        {
                            error!("error logging trace: {:?}", e);
                        }
                    });
                }
            } else {
                info!("client_trace: {}", message);
            }
            client.traces += 1;
            Ok(ClientUpdate::Traced)
        } else {
            Err("too many traces")
        }
    }

    /// Handles an arbitrary [`ClientRequest`].
    fn handle_client_request(
        &mut self,
        server_id: ServerId,
        arena_token: ArenaToken,
        player_id: PlayerId,
        request: ClientRequest,
        players: &PlayerRepo<G>,
        metrics: &mut MetricRepo<G>,
        plasma: &PlasmaClient,
    ) -> Result<ClientUpdate, &'static str> {
        match request {
            ClientRequest::Login(session_token) => Self::login(
                players,
                server_id,
                arena_token,
                player_id,
                session_token,
                plasma,
            ),
            ClientRequest::SetAlias(alias) => Self::set_alias(player_id, alias, players),
            ClientRequest::TallyAd(ad_type) => Self::tally_ad(player_id, ad_type, players, metrics),
            ClientRequest::TallyFps(fps) => Self::tally_fps(player_id, fps, players),
            ClientRequest::Trace { message } => self.trace(player_id, message, players, metrics),
        }
    }

    /// Handles request made by real player.
    #[allow(clippy::too_many_arguments)]
    fn handle_observer_request(
        &mut self,
        player_id: PlayerId,
        request: Request<G::GameRequest>,
        service: &mut G,
        realm_name: Option<RealmName>,
        arena_token: ArenaToken,
        server_id: ServerId,
        players: &mut PlayerRepo<G>,
        #[cfg(feature = "teams")] teams: &mut TeamRepo<G>,
        chat: &mut ChatRepo<G>,
        invitations: &mut InvitationRepo<G>,
        metrics: &mut MetricRepo<G>,
        plasma: &PlasmaClient,
    ) -> Result<Option<Update<G::GameUpdate>>, &'static str> {
        match request {
            // Goes first (fast path).
            Request::Game(command) => {
                Self::handle_game_command(player_id, command, service, &*players)
                    .map(|u| u.map(Update::Game))
            }
            Request::Client(request) => self
                .handle_client_request(
                    server_id,
                    arena_token,
                    player_id,
                    request,
                    &*players,
                    metrics,
                    plasma,
                )
                .map(|u| Some(Update::Client(u))),
            Request::Chat(request) => chat
                .handle_chat_request(
                    player_id,
                    request,
                    service,
                    players,
                    #[cfg(feature = "teams")]
                    teams,
                    metrics,
                )
                .map(|u| Some(Update::Chat(u))),
            Request::Invitation(request) => invitations
                .handle_invitation_request(player_id, request, realm_name, server_id, players)
                .map(|u| Some(Update::Invitation(u))),
            Request::Player(request) => players
                .handle_player_request(player_id, request, metrics)
                .map(|u| Some(Update::Player(u))),
            #[cfg(feature = "teams")]
            Request::Team(request) => teams
                .handle_team_request(player_id, request, players)
                .map(|u| Some(Update::Team(u))),
            #[cfg(not(feature = "teams"))]
            Request::Team(_) => Err("unhandled teams request"),
        }
    }

    /// Record network round-trip-time measured by websocket for statistical purposes.
    fn handle_observer_rtt(&mut self, player_id: PlayerId, rtt: u16, players: &PlayerRepo<G>) {
        let mut player = match players.borrow_player_mut(player_id) {
            Some(player) => player,
            None => return,
        };

        let client = match player.client_mut() {
            Some(client) => client,
            None => {
                debug_assert!(false);
                return;
            }
        };

        client.metrics.rtt = Some(rtt);
    }
}

/// Don't let bad values sneak in.
fn sanitize_tps(tps: f32) -> Option<f32> {
    tps.is_finite().then_some(tps.clamp(0.0, 144.0))
}

/// Data stored per client (a.k.a websocket a.k.a. real player).
#[derive(Debug)]
pub struct PlayerClientData<G: GameArenaService> {
    /// Authentication.
    //pub(crate) session_id: SessionId,
    token: Token,
    /// Alias chosen by player.
    pub(crate) alias: PlayerAlias,
    /// Connection state.
    pub(crate) status: ClientStatus<G>,
    /// Plasma session id.
    pub(crate) session_token: Option<SessionToken>,
    /// Plasma user id.
    pub(crate) user_id: Option<UserId>,
    /// Plasma nick name.
    pub(crate) nick_name: Option<NickName>,
    /// Ip address.
    pub(crate) ip_address: IpAddr,
    /// Is admin (developer).
    pub admin: bool,
    /// Is moderator for in-game chat?
    pub moderator: bool,
    /// Metrics-related information associated with each client.
    pub(crate) metrics: ClientMetricData<G>,
    /// Invitation-related information associated with each client.
    pub(crate) invitation: ClientInvitationData,
    /// Chat-related information associated with each client.
    pub(crate) chat: ClientChatData,
    /// Team-related information associated with each client.
    #[cfg(feature = "teams")]
    pub(crate) team: ClientTeamData,
    /// Players this client has reported.
    pub(crate) reported: HashSet<PlayerId>,
    /// Number of times sent error trace (in order to limit abuse).
    pub(crate) traces: u8,
    /// Game specific client data. Manually serialized
    pub(crate) data: AtomicRefCell<G::ClientData>,
}

#[derive(Debug)]
pub(crate) enum ClientStatus<G: GameArenaService> {
    /// Pending: Initial state. Visit not started yet. Can be forgotten after expiry.
    Pending { expiry: Instant },
    /// Connected and in game. Transitions to limbo if the connection is lost.
    Connected { observer: ClientAddr<G> },
    /// Disconnected but still in game (and visit still in progress).
    /// - Transitions to connected if a new connection is established.
    /// - Transitions to leaving limbo after expiry.
    Limbo { expiry: Instant },
    /// Disconnected and not in game (but visit still in progress).
    /// - Transitions to connected if a new connection is established.
    /// - Transitions to stale after finished leaving game.
    LeavingLimbo { since: Instant },
}

impl<G: GameArenaService> PlayerClientData<G> {
    pub(crate) fn new(
        metrics: ClientMetricData<G>,
        session_token: Option<SessionToken>,
        invitation: Option<InvitationDto>,
        ip: IpAddr,
    ) -> Self {
        Self {
            token: thread_rng().gen(),
            alias: G::default_alias(),
            status: ClientStatus::Pending {
                expiry: Instant::now() + Duration::from_secs(10),
            },
            session_token,
            nick_name: None,
            user_id: None,
            ip_address: ip,
            admin: false,
            moderator: false,
            metrics,
            invitation: ClientInvitationData::new(invitation),
            chat: ClientChatData::default(),
            #[cfg(feature = "teams")]
            team: ClientTeamData::default(),
            reported: Default::default(),
            traces: 0,
            data: AtomicRefCell::new(G::ClientData::default()),
        }
    }

    /// Requires mutable self, but as a result, guaranteed not to panic.
    pub fn data(&mut self) -> &G::ClientData {
        &*self.data.get_mut()
    }

    /// Infallible way of getting mutable client data.
    pub fn data_mut(&mut self) -> &mut G::ClientData {
        self.data.get_mut()
    }
}

/// Handle client messages.
impl<G: GameArenaService> Handler<ObserverMessage<Request<G::GameRequest>, Update<G::GameUpdate>>>
    for Infrastructure<G>
{
    type Result = ();

    fn handle(
        &mut self,
        msg: ObserverMessage<Request<G::GameRequest>, Update<G::GameUpdate>>,
        _ctx: &mut Self::Context,
    ) {
        let Some(context_service) = self.arenas.get_mut(msg.realm_name) else {
            match msg.body {
                ObserverMessageBody::Register { observer, .. } => {
                    let _ = observer.send(ObserverUpdate::Close);
                },
                _ => {
                    // should have already been closed.
                }
            }
            return;
        };

        match msg.body {
            ObserverMessageBody::Register {
                player_id,
                observer,
                ..
            } => self.clients.register(
                player_id,
                observer,
                &mut context_service.context.players,
                #[cfg(feature = "teams")]
                &mut context_service.context.teams,
                &context_service.context.chat,
                &self.leaderboard,
                &context_service.context.liveboard,
                &mut self.metrics,
                &self.system,
                self.server_id,
                msg.realm_name,
                &mut context_service.service,
            ),
            ObserverMessageBody::Unregister {
                player_id,
                observer,
            } => self
                .clients
                .unregister(player_id, observer, &context_service.context.players),
            ObserverMessageBody::Request { player_id, request } => {
                let context = &mut context_service.context;
                let service = &mut context_service.service;
                match self.clients.handle_observer_request(
                    player_id,
                    request,
                    service,
                    msg.realm_name,
                    context.token,
                    self.server_id,
                    &mut context.players,
                    #[cfg(feature = "teams")]
                    &mut context.teams,
                    &mut context.chat,
                    &mut self.invitations,
                    &mut self.metrics,
                    &self.plasma,
                ) {
                    Ok(Some(message)) => {
                        let player = match context.players.borrow_player_mut(player_id) {
                            Some(player) => player,
                            None => {
                                debug_assert!(false);
                                return;
                            }
                        };

                        let client = match player.client() {
                            Some(client) => client,
                            None => {
                                debug_assert!(false);
                                return;
                            }
                        };

                        if let ClientStatus::Connected { observer } = &client.status {
                            let _ = observer.send(ObserverUpdate::Send { message });
                        } else {
                            debug_assert!(false, "impossible due to synchronous nature of code");
                        }
                    }
                    Ok(None) => {}
                    Err(s) => {
                        warn!("observer request resulted in {}", s);
                    }
                }
            }
            ObserverMessageBody::RoundTripTime { player_id, rtt } => self
                .clients
                .handle_observer_rtt(player_id, rtt, &context_service.context.players),
        }
    }
}

#[derive(Message)]
#[rtype(result = "Result<(Option<RealmName>, PlayerId), &'static str>")]
pub struct Authenticate {
    /// Client ip address.
    pub ip_address: IpAddr,
    /// User agent.
    pub user_agent_id: Option<UserAgentId>,
    /// Referrer.
    pub referrer: Option<Referrer>,
    /// Desired realm id.
    pub realm_name: Option<RealmName>,
    /// Last valid credentials.
    pub player_id_token: Option<(PlayerId, Token)>,
    /// Session id.
    pub session_token: Option<SessionToken>,
    /// Invitation?
    pub invitation_id: Option<InvitationId>,
    /// Previous cohort.
    pub cohort_id: Option<CohortId>,
    /// When joined the system (maybe now).
    pub date_created: UnixTime,
}

impl<G: GameArenaService> Handler<Authenticate> for Infrastructure<G> {
    type Result = Result<(Option<RealmName>, PlayerId), &'static str>;

    fn handle(&mut self, msg: Authenticate, _ctx: &mut ActorContext<Self>) -> Self::Result {
        let clients = &mut self.clients;

        if clients
            .authenticate_rate_limiter
            .should_limit_rate(msg.ip_address)
        {
            // Should only log IP of malicious actors.
            warn!("IP {:?} was rate limited", msg.ip_address);
            return Err("rate limit exceeded");
        }

        let realm_name = msg.realm_name;
        let Some(context_service) = self.arenas.get_mut(realm_name) else {
            return Err("no such arena");
        };
        let arena_token = context_service.context.token;

        let invitations = &self.invitations;
        let invitation = msg
            .invitation_id
            .and_then(|id| invitations.get(id).cloned());
        let invitation_dto = invitation.map(|i| InvitationDto {
            player_id: i.player_id,
        });

        let player_id = if let Some(existing) = msg
            .player_id_token
            .filter(|(player_id, token)| {
                context_service
                    .context
                    .players
                    .borrow_player(*player_id)
                    .and_then(|p| p.client().map(|c| c.token == *token))
                    .unwrap_or(false)
            })
            .map(|(player_id, _)| player_id)
        {
            existing
        } else {
            loop {
                let player_id = PlayerId(generate_id());
                if !context_service.context.players.contains(player_id) {
                    break player_id;
                }
            }
        };

        match context_service.context.players.players.entry(player_id) {
            Entry::Occupied(mut occupied) => {
                if let Some(client) = occupied.get_mut().borrow_player_mut().client_mut() {
                    // Update the referrer, such that the correct snippet may be served.
                    client.metrics.referrer = msg.referrer.or(client.metrics.referrer);
                } else {
                    debug_assert!(false, "impossible to be a bot since session was valid");
                }
            }
            Entry::Vacant(vacant) => {
                let client_metric_data = ClientMetricData::new(&msg);

                let client = PlayerClientData::new(
                    client_metric_data,
                    msg.session_token,
                    invitation_dto,
                    msg.ip_address,
                );

                if let Some(session_token) = msg.session_token {
                    self.plasma.do_request(PlasmaRequestV1::RegisterPlayer {
                        game_id: G::GAME_ID,
                        server_id: self.server_id,
                        realm_name: None,
                        // TODO.
                        arena_token,
                        player_id,
                        session_token,
                    });
                }

                let pd = PlayerData::new(player_id, Some(Box::new(client)));
                let pt = Arc::new(PlayerTuple::new(pd));
                vacant.insert(pt);
            }
        }

        Ok((realm_name, player_id))
    }
}
