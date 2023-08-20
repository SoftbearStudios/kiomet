use crate::{game_service::GameArenaService, infrastructure::Infrastructure};
use actix::{Handler, Recipient};
use axum::http::Method;
use core_protocol::{
    ArenaToken, ClientHash, GameId, PlasmaRequest, PlasmaRequestV1, PlasmaUpdate, PlasmaUpdateV1,
    RealmName, RegionId, ServerId, ServerNumber, ServerRole,
};
use log::{info, warn};
use reqwest::Client;
use std::collections::HashMap;
use std::future::Future;
use std::net::Ipv4Addr;
use std::sync::atomic::AtomicU8;
use std::sync::Mutex;
use std::time::Instant;
use std::{
    sync::atomic::{AtomicU64, Ordering},
    time::Duration,
};

pub(crate) struct PlasmaClient {
    redirect_server_number: &'static AtomicU8,
    realm_routes: &'static Mutex<HashMap<RealmName, ServerNumber>>,
    pub server_token: &'static AtomicU64,
    pub role: ServerRole,
    client: Client,
    infrastructure: Option<Recipient<PlasmaUpdate>>,
    /// Last outbound heartbeat time.
    last_heartbeat: Option<Instant>,
    /// Last message from plasma.
    last_message: Option<Instant>,
}

impl PlasmaClient {
    pub(crate) fn new(
        redirect_server_number: &'static AtomicU8,
        realm_routes: &'static Mutex<HashMap<RealmName, ServerNumber>>,
        server_token: &'static AtomicU64,
    ) -> Self {
        Self {
            redirect_server_number,
            realm_routes,
            server_token,
            role: ServerRole::Unlisted,
            client: Client::builder()
                .timeout(Duration::from_secs(15))
                .build()
                .unwrap(),
            infrastructure: None,
            last_heartbeat: None,
            last_message: None,
        }
    }

    pub(crate) fn set_infrastructure(&mut self, infrastructure: Recipient<PlasmaUpdate>) {
        self.infrastructure = Some(infrastructure);
    }

    pub(crate) fn update(
        &mut self,
        game_id: GameId,
        server_id: ServerId,
        arena_token: ArenaToken,
        region_id: Option<RegionId>,
        cpu: f32,
        ram: f32,
        healthy: bool,
        player_count: u32,
        client_hash: ClientHash,
        ipv4_address: Option<Ipv4Addr>,
    ) {
        let now = Instant::now();
        if self
            .last_heartbeat
            .map(|last_poll| now.saturating_duration_since(last_poll) < Duration::from_secs(60))
            .unwrap_or(false)
        {
            return;
        }
        self.last_heartbeat = Some(now);
        let mut requests = Vec::new();
        if self
            .last_message
            .map(|last_message| {
                now.saturating_duration_since(last_message) > Duration::from_secs(75)
            })
            .unwrap_or(true)
        {
            // Fail-open to avoid locking players out.
            self.set_role(ServerRole::Unlisted);

            requests.push(PlasmaRequestV1::Authenticate { game_id, server_id });
            requests.push(PlasmaRequestV1::RegisterServer {
                game_id,
                server_id,
                region_id,
                client_hash,
                ipv4_address,
            });
            requests.push(PlasmaRequestV1::RegisterArena {
                game_id,
                server_id,
                arena_token,
                realm_name: None,
            });
        }
        requests.push(PlasmaRequestV1::Heartbeat {
            game_id,
            server_id,
            unhealthy: !healthy,
            cpu,
            ram,
            player_count,
            client_hash,
        });
        self.do_requests(requests);
    }

    pub(crate) fn request(
        &self,
        request: PlasmaRequestV1,
    ) -> impl Future<Output = Result<PlasmaUpdate, ()>> + Send {
        Self::request_impl(request, &self.client, self.server_token)
    }

    pub(crate) fn request_impl(
        request: PlasmaRequestV1,
        client: &reqwest::Client,
        token: &'static AtomicU64,
    ) -> impl Future<Output = Result<PlasmaUpdate, ()>> + Send + 'static {
        info!("executing plasma request: {request:?}");
        let request = client
            .request(Method::POST, "http://example.com/plasma")
            .bearer_auth(token.load(Ordering::Relaxed))
            .json(&PlasmaRequest::V1(request));

        async move {
            match request.send().await {
                Ok(r) => {
                    let status = r.status();
                    if status.is_success() {
                        match r.json().await {
                            Ok(response) => {
                                log::info!("{response:?} (code {status})");
                                return Ok(response);
                            }
                            Err(e) => {
                                log::error!("{e}");
                            }
                        }
                    } else {
                        match r.text().await {
                            Ok(body) => {
                                log::warn!("{body} (code {status})");
                            }
                            Err(e) => {
                                log::error!("{e} (code {status})");
                            }
                        }
                    }
                }
                Err(e) => {
                    log::error!("{e}")
                }
            }
            Err(())
        }
    }

    pub(crate) fn do_request(&self, request: PlasmaRequestV1) {
        let request = self.request(request);
        let infrastructure = self.infrastructure.clone();

        tokio::spawn(async {
            if let Ok(update) = request.await {
                if let Some(infrastructure) = infrastructure {
                    infrastructure.do_send(update);
                } else {
                    debug_assert!(false);
                }
            }
        });
    }

    pub(crate) fn do_requests(&self, requests: Vec<PlasmaRequestV1>) {
        let client = self.client.clone();
        let token = self.server_token;
        let infrastructure = self.infrastructure.clone();

        tokio::spawn(async move {
            for request in requests {
                if let Ok(update) = Self::request_impl(request, &client, token).await {
                    if let Some(infrastructure) = infrastructure.as_ref() {
                        infrastructure.do_send(update);
                    } else {
                        debug_assert!(false);
                    }
                }
            }
        });
    }

    fn set_role(&mut self, role: ServerRole) {
        self.role = role;
        self.redirect_server_number.store(
            role.redirect().map(|s| s.0.get()).unwrap_or(0),
            Ordering::Relaxed,
        );
    }
}

impl<G: GameArenaService> Handler<PlasmaUpdate> for Infrastructure<G> {
    type Result = ();

    fn handle(&mut self, response: PlasmaUpdate, _: &mut Self::Context) -> Self::Result {
        println!("received plasma update {response:?}");
        self.plasma.last_message = Some(Instant::now());

        #[allow(clippy::infallible_destructuring_match)]
        let updates = match response {
            PlasmaUpdate::V1(updates) => updates,
        };

        for update in Vec::from(updates) {
            match update {
                PlasmaUpdateV1::ConfigServer { token, role } => {
                    if let Some(token) = token {
                        self.plasma
                            .server_token
                            .store(token.0.get(), Ordering::Relaxed);
                    }
                    if let Some(role) = role {
                        self.plasma.set_role(role);
                    }
                }
                PlasmaUpdateV1::ConfigPlayer {
                    realm_name,
                    player_id,
                    session_token,
                    user_id,
                    admin,
                    moderator,
                    nick_name,
                    ..
                } => {
                    if let Some(context_service) = self.arenas.get_mut(realm_name) {
                        if let Some(mut player) =
                            context_service.context.players.borrow_player_mut(player_id)
                        {
                            if let Some(client) = player.client_mut() {
                                if client.session_token == Some(session_token)
                                    || client.user_id == Some(user_id)
                                {
                                    client.user_id = Some(user_id);
                                    client.nick_name = nick_name;
                                    client.admin = admin;
                                    client.moderator = moderator;
                                    info!(
                                        "set moderator status of {session_token:?} to {moderator}"
                                    );
                                    return;
                                } else {
                                    warn!("user_id/session_id didn't match");
                                }
                            }
                        }
                    }
                }
                PlasmaUpdateV1::Leaderboards {
                    leaderboards,
                    realm_name,
                } => {
                    for (period_id, scores) in Vec::from(leaderboards) {
                        if let Some(realm) = self.arenas.get_mut(realm_name) {
                            realm.context.leaderboard.put_leaderboard(period_id, scores);
                        }
                    }
                }
                PlasmaUpdateV1::Leaderboard {
                    period_id,
                    scores,
                    realm_name,
                } => {
                    if let Some(realm) = self.arenas.get_mut(realm_name) {
                        realm.context.leaderboard.put_leaderboard(period_id, scores);
                    }
                }
                PlasmaUpdateV1::Servers { servers } => {
                    self.system.servers = servers;
                }
                PlasmaUpdateV1::Realms { added, removed } => {
                    let mut routes = self.plasma.realm_routes.lock().unwrap();
                    for removed in removed.iter() {
                        routes.remove(removed);
                    }
                    for added in added.iter() {
                        if let Some(server_number) = added.server_number {
                            routes.insert(added.realm_name, server_number);
                        }
                    }
                }
                _ => {}
            }
        }
        // warn!("unhandled plasma update {update:?}");
    }
}
