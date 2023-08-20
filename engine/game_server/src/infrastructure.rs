// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::admin::AdminRepo;
use crate::arena::ArenaRepo;
use crate::client::ClientRepo;
use crate::context_service::ContextService;
use crate::game_service::GameArenaService;
use crate::invitation::InvitationRepo;
use crate::leaderboard::LeaderboardRepo;
use crate::metric::MetricRepo;
use crate::plasma::PlasmaClient;
use crate::system::SystemRepo;
use actix::AsyncContext;
use actix::{Actor, Context as ActorContext};
use core_protocol::id::{ClientHash, RegionId, ServerId};
use core_protocol::{PlasmaRequestV1, PlasmaUpdate, RealmName, ServerNumber};
use futures::stream::FuturesUnordered;
use log::{error, info};
use minicdn::MiniCdn;
use server_util::health::Health;
use server_util::rate_limiter::RateLimiterProps;
use std::collections::HashMap;
use std::future::Future;
use std::net::Ipv4Addr;
use std::pin::Pin;
use std::sync::atomic::{AtomicU64, AtomicU8};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

/// An entire game server.
pub struct Infrastructure<G: GameArenaService> {
    /// What server/region does this infrastructure represent?
    pub(crate) server_id: ServerId,
    pub(crate) ipv4_address: Option<Ipv4Addr>,
    pub(crate) region_id: Option<RegionId>,

    /// API.
    pub(crate) plasma: PlasmaClient,
    pub(crate) system: SystemRepo<G>,

    /// Game specific stuff.
    pub(crate) arenas: ArenaRepo<G>,
    /// Game client information.
    pub(crate) clients: ClientRepo<G>,
    /// Shared invitations.
    pub(crate) invitations: InvitationRepo<G>,
    /// Shared admin interface.
    pub(crate) admin: AdminRepo<G>,
    /// Shared metrics.
    pub(crate) metrics: MetricRepo<G>,

    /// Monitoring.
    pub(crate) health: Health,

    /// Drop missed updates.
    last_update: Instant,
}

impl<G: GameArenaService> Actor for Infrastructure<G> {
    type Context = ActorContext<Self>;

    fn started(&mut self, ctx: &mut Self::Context) {
        info!("infrastructure started");

        // TODO: Investigate whether this only affects performance or can affect correctness.
        ctx.set_mailbox_capacity(50);

        ctx.run_interval(Duration::from_secs_f32(G::TICK_PERIOD_SECS), Self::update);

        self.plasma.set_infrastructure(ctx.address().recipient());
    }

    fn stopped(&mut self, _ctx: &mut Self::Context) {
        error!("infrastructure stopped");

        let futures = FuturesUnordered::<
            Pin<Box<dyn Future<Output = Result<PlasmaUpdate, ()>> + Send>>,
        >::new();
        futures.push(Box::pin(self.plasma.request(
            PlasmaRequestV1::UnregisterServer {
                game_id: G::GAME_ID,
                server_id: self.server_id,
            },
        )));

        use futures::StreamExt;
        let fut = futures.into_future();

        tokio::spawn(async {
            let _ = fut.await;

            // A process without this actor running should be restarted immediately.
            std::process::exit(0);
        });
    }
}

impl<G: GameArenaService> Infrastructure<G> {
    /// new returns a game server with the specified parameters.
    #[allow(clippy::too_many_arguments)]
    pub async fn new(
        server_id: ServerId,
        redirect_server_number: &'static AtomicU8,
        realm_routes: &'static Mutex<HashMap<RealmName, ServerNumber>>,
        client_hash: ClientHash,
        ipv4_address: Option<Ipv4Addr>,
        region_id: Option<RegionId>,
        min_bots: Option<usize>,
        max_bots: Option<usize>,
        bot_percent: Option<usize>,
        chat_log: Option<String>,
        trace_log: Option<String>,
        game_client: Arc<RwLock<MiniCdn>>,
        server_token: &'static AtomicU64,
        client_authenticate: RateLimiterProps,
    ) -> Self {
        Self {
            server_id,
            ipv4_address,
            region_id,
            clients: ClientRepo::new(trace_log, client_authenticate),
            plasma: PlasmaClient::new(redirect_server_number, realm_routes, server_token),
            system: SystemRepo::new(),
            admin: AdminRepo::new(game_client, client_hash),
            arenas: ArenaRepo::new(ContextService::new(
                min_bots,
                max_bots,
                bot_percent,
                chat_log,
            )),
            health: Health::default(),
            invitations: InvitationRepo::default(),
            metrics: MetricRepo::new(),
            last_update: Instant::now(),
        }
    }

    /// Call once every tick.
    pub fn update(&mut self, ctx: &mut <Infrastructure<G> as Actor>::Context) {
        let now = Instant::now();
        if now.duration_since(self.last_update) < Duration::from_secs_f32(G::TICK_PERIOD_SECS * 0.5)
        {
            // Less than half a tick elapsed. Drop this update on the floor, to avoid jerking.
            return;
        }
        self.last_update = now;

        let server_delta = self.system.delta();
        for (_, context_service) in self.arenas.iter_mut() {
            context_service.update(
                &mut self.clients,
                &mut self.invitations,
                &mut self.metrics,
                &server_delta,
                self.server_id,
                &self.plasma,
            );
        }

        self.health.record_tick(G::TICK_PERIOD_SECS);

        // These are all rate-limited internally.
        LeaderboardRepo::update_to_plasma(self);
        MetricRepo::update_to_plasma(self, ctx);
        self.plasma.update(
            G::GAME_ID,
            self.server_id,
            self.arenas.main().context.token,
            self.region_id,
            self.health.cpu() + self.health.cpu_steal(),
            self.health.ram(),
            self.health.healthy(),
            self.arenas.main().context.players.real_players_live as u32,
            self.admin.client_hash,
            self.ipv4_address,
        );
    }
}
