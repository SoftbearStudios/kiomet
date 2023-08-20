// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

//! The game server has authority over all game logic. Clients are served the client, which connects
//! via web_socket.

use crate::client::Authenticate;
use crate::game_service::GameArenaService;
use crate::infrastructure::Infrastructure;
use crate::net::ip::{get_own_public_ip, ip_to_region_id};
use crate::options::Options;
use crate::static_files::{static_size_and_hash, StaticFilesHandler};
use crate::system::SystemRequest;
use actix::Actor;
use axum::body::{boxed, Empty, HttpBody};
use axum::extract::ws::{CloseCode, CloseFrame, Message};
use axum::extract::{ConnectInfo, FromRequestParts, Query, TypedHeader, WebSocketUpgrade};
use axum::headers::authorization::Bearer;
use axum::headers::Authorization;
use axum::http::header::CACHE_CONTROL;
use axum::http::uri::{Authority, Scheme};
use axum::http::{HeaderValue, Method, StatusCode, Uri};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use core_protocol::rpc::{Request, SystemQuery, Update, WebSocketQuery};
use core_protocol::{get_unix_time_now, AdminRequest, AdminUpdate, UnixTime};
use core_protocol::{id::*, PlasmaUpdate, RealmName};
use futures::pin_mut;
use futures::SinkExt;
use log::{debug, error, info, warn};
use minicdn::MiniCdn;
use rand::{thread_rng, Rng};
use server_util::http::limit_content_length;
use server_util::ip_rate_limiter::IpRateLimiter;
use server_util::observer::{ObserverMessage, ObserverMessageBody, ObserverUpdate};
use server_util::os::set_open_file_limit;
use server_util::rate_limiter::{RateLimiterProps, RateLimiterState};
use server_util::user_agent::UserAgent;
use std::{
    collections::HashMap,
    convert::TryInto,
    fs::File,
    io::Write,
    net::{IpAddr, SocketAddr},
    num::NonZeroU64,
    str::FromStr,
    sync::{
        atomic::{AtomicU64, AtomicU8, Ordering},
        Arc, Mutex, RwLock,
    },
    time::{Duration, Instant},
};
use structopt::StructOpt;
use tower::ServiceBuilder;
use tower_http::cors::CorsLayer;

/// 0 is no redirect.
static REDIRECT_TO_SERVER_ID: AtomicU8 = AtomicU8::new(0);

/// Admin password.
static SERVER_TOKEN: AtomicU64 = AtomicU64::new(0);

lazy_static::lazy_static! {
    static ref REALM_ROUTES: Mutex<HashMap<RealmName, ServerNumber>> = Mutex::default();
    // Will be overwritten first thing.
    static ref HTTP_RATE_LIMITER: Mutex<IpRateLimiter> = Mutex::new(IpRateLimiter::new_bandwidth_limiter(1, 0));
}

struct Authenticated;

impl Authenticated {
    fn validate(value: &str) -> bool {
        value
            .parse::<u64>()
            .map(|parsed| parsed != 0 && parsed == SERVER_TOKEN.load(Ordering::Relaxed))
            .unwrap_or(false)
    }
}

enum AuthenticatedError {
    Missing,
    Invalid,
}

impl IntoResponse for AuthenticatedError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            match self {
                Self::Missing => "missing key",
                Self::Invalid => "invalid key",
            },
        )
            .into_response()
    }
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for Authenticated
where
    S: Send + Sync,
{
    type Rejection = AuthenticatedError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let bearer = TypedHeader::<Authorization<Bearer>>::from_request_parts(parts, state)
            .await
            .map_err(|_| AuthenticatedError::Missing)?;
        if Self::validate(bearer.0.token()) {
            Ok(Self)
        } else {
            warn!(
                "invalid key {} (correct is {})",
                bearer.0.token(),
                SERVER_TOKEN.load(Ordering::Relaxed)
            );
            Err(AuthenticatedError::Invalid)
        }
    }
}

struct ExtractRealmName(RealmName);

impl ExtractRealmName {
    fn parse(domain: &str) -> Option<RealmName> {
        if domain.bytes().filter(|&b| b == b'.').count() < 2 {
            return None;
        }
        domain
            .split('.')
            .next()
            .filter(|&host| usize::from_str(host).is_err() && host != "www")
            .and_then(|host| RealmName::from_str(host).ok())
    }
}

enum ExtractRealmNameError {
    Missing,
    Invalid,
}

impl IntoResponse for ExtractRealmNameError {
    fn into_response(self) -> Response {
        (
            StatusCode::UNAUTHORIZED,
            match self {
                Self::Missing => "missing realm name",
                Self::Invalid => "invalid realm name",
            },
        )
            .into_response()
    }
}

#[async_trait::async_trait]
impl<S> FromRequestParts<S> for ExtractRealmName
where
    S: Send + Sync,
{
    type Rejection = ExtractRealmNameError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let host = TypedHeader::<axum::headers::Host>::from_request_parts(parts, state)
            .await
            .map_err(|_| ExtractRealmNameError::Missing)?;
        if let Some(realm_name) = Self::parse(host.hostname()) {
            Ok(Self(realm_name))
        } else {
            Err(ExtractRealmNameError::Invalid)
        }
    }
}

pub fn entry_point<G: GameArenaService>(game_client: MiniCdn, browser_router: bool)
where
    <G as GameArenaService>::GameUpdate: std::fmt::Debug,
{
    actix::System::new().block_on(async move {
        SERVER_TOKEN.store({
            thread_rng().gen::<NonZeroU64>().get()
        }, Ordering::Relaxed);

        let options = Options::from_args();

        crate::log::init_logger(&options);

        match set_open_file_limit(16384) {
            Ok(limit) => info!("set open file limit to {}", limit),
            Err(e) => error!("could not set open file limit: {}", e)
        }

        #[allow(unused)]
        let (http_port, https_port) = options.http_and_https_ports();

        let (static_size, static_hash) = static_size_and_hash(&game_client);
        let bandwidth_burst = options.bandwidth_burst(static_size);

        *HTTP_RATE_LIMITER.lock().unwrap() =
            IpRateLimiter::new_bandwidth_limiter(options.http_bandwidth_limit, bandwidth_burst);

        let certificate_private_key_paths = options.certificate_private_key_paths();

        let server_id = if let Some(number) = ServerNumber::new(options.server_id) {
            ServerId{
                number,
                kind: ServerKind::Cloud
            }
        } else {
            ServerId{number: ServerNumber(thread_rng().gen()),
                kind: ServerKind::Local,
            }
        };
        let ip_address = if let Some(ip_address) = options.ip_address {
            Some(ip_address)
        } else {
            get_own_public_ip().await
        };
        let region_id = if let Some(region_id) = options.region_id {
            Some(region_id)
        } else {
            ip_address.and_then(|ip| ip_to_region_id(ip))
        };

        let game_client = Arc::new(RwLock::new(game_client));

        let srv = Infrastructure::<G>::start(
            Infrastructure::new(
                server_id,
                &REDIRECT_TO_SERVER_ID,
                &REALM_ROUTES,
                static_hash,
                ip_address.and_then(|ip| if let IpAddr::V4(ipv4_address) = ip {
                    Some(ipv4_address)
                } else {
                    None
                }),
                region_id,
                options.min_bots,
                options.max_bots,
                options.bot_percent,
                options.chat_log,
                options.trace_log,
                Arc::clone(&game_client),
                &SERVER_TOKEN,
                RateLimiterProps::new(
                    Duration::from_secs(options.client_authenticate_rate_limit),
                    options.client_authenticate_burst,
                ),
            )
            .await,
        );

        // Manual profile.
        if false {
            let future = srv.send(AdminRequest::RequestProfile);
            tokio::spawn(async move {
                let result = future.await;
                if let Ok(Ok(AdminUpdate::ProfileRequested(profile))) = result {
                    if let Ok(mut file) = File::create("/tmp/server_profile.xml") {
                        if file.write_all(profile.as_bytes()).is_ok() {
                            info!("saved profile");
                        }
                    }
                }
            });
        }

        let ws_srv = srv.to_owned();
        let admin_srv = srv.to_owned();
        let plasma_srv = srv.to_owned();
        let system_srv = srv.to_owned();

        let admin_router = post(
            move |_: Authenticated, request: Json<AdminRequest>| {
                let srv_clone_admin = admin_srv.clone();

                async move {
                    match srv_clone_admin.send(request.0).await {
                        Ok(result) => match result {
                            Ok(update) => {
                                Ok(Json(update))
                            }
                            Err(e) => Err((StatusCode::BAD_REQUEST, String::from(e)).into_response()),
                        },
                        Err(e) => {
                            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
                        }
                    }
                }
            }
        );

        let app = Router::new()
            .fallback_service(get(StaticFilesHandler{cdn: game_client, prefix: "", browser_router}))
            .route("/ws", axum::routing::get(async move |upgrade: WebSocketUpgrade, ConnectInfo(addr): ConnectInfo<SocketAddr>, user_agent: Option<TypedHeader<axum::headers::UserAgent>>, realm_name: Option<ExtractRealmName>, Query(query): Query<WebSocketQuery>| {
                let user_agent_id = user_agent
                    .map(|h| UserAgent::new(h.as_str()))
                    .and_then(UserAgent::into_id);

                let now = get_unix_time_now();

                let authenticate = Authenticate {
                    ip_address: addr.ip(),
                    referrer: query.referrer,
                    user_agent_id,
                    realm_name: realm_name.map(|e| e.0),
                    player_id_token: query.player_id.zip(query.token),
                    session_token: query.session_token,
                    date_created: query.date_created.filter(|&d| d > 1680570365768 && d <= now).unwrap_or(now),
                    invitation_id: query.invitation_id,
                    cohort_id: query.cohort_id,
                };

                const MAX_MESSAGE_SIZE: usize = 32768;
                const TIMER_SECONDS: u64 = 10;
                const TIMER_DURATION: Duration = Duration::from_secs(TIMER_SECONDS);
                const WEBSOCKET_HARD_TIMEOUT: Duration = Duration::from_secs(TIMER_SECONDS * 2);

                match ws_srv.send(authenticate).await {
                    Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
                    Ok(result) => match result {
                        // Currently, if authentication fails, it was due to rate limit.
                        Err(e) => Err((StatusCode::TOO_MANY_REQUESTS, e).into_response()),
                        Ok((realm_name, player_id)) => Ok(upgrade
                            .max_frame_size(MAX_MESSAGE_SIZE)
                            .max_message_size(MAX_MESSAGE_SIZE)
                            .write_buffer_size(0)
                            .max_write_buffer_size(MAX_MESSAGE_SIZE * 32)
                            .on_upgrade(async move |mut web_socket| {
                            let (server_sender, mut server_receiver) = tokio::sync::mpsc::unbounded_channel::<ObserverUpdate<Update<G::GameUpdate>>>();

                            ws_srv.do_send(ObserverMessage{
                                realm_name,
                                body: ObserverMessageBody::<Request<G::GameRequest>, Update<G::GameUpdate>>::Register {
                                    player_id,
                                    observer: server_sender.clone(),
                                }
                            });

                            let keep_alive = tokio::time::sleep(TIMER_DURATION);
                            let mut last_activity = Instant::now();
                            let mut rate_limiter = RateLimiterState::default();
                            let mut measure_rtt_ping_governor = RateLimiterState::default();
                            const RATE: RateLimiterProps = RateLimiterProps::const_new(Duration::from_millis(80), 5);
                            const MEASURE_RTT_PING: RateLimiterProps = RateLimiterProps::const_new(Duration::from_secs(60), 0);

                            pin_mut!(keep_alive);

                            // For signaling what type of close frame should be sent, if any.
                            // See https://github.com/tokio-rs/axum/issues/1061
                            const NORMAL_CLOSURE: Option<CloseCode> = Some(1000);
                            const PROTOCOL_ERROR: Option<CloseCode> = Some(1002);
                            const SILENT_CLOSURE: Option<CloseCode> = None;

                            let closure = loop {
                                tokio::select! {
                                    web_socket_update = web_socket.recv() => {
                                        match web_socket_update {
                                            Some(result) => match result {
                                                Ok(message) => {
                                                    last_activity = Instant::now();
                                                    keep_alive.as_mut().reset((last_activity + TIMER_DURATION).into());

                                                    match message {
                                                        Message::Binary(binary) => {
                                                            if rate_limiter.should_limit_rate_with_now(&RATE, last_activity) {
                                                                warn!("rate-limiting client binary");
                                                                continue;
                                                            }

                                                            match core_protocol::bitcode::decode(binary.as_ref())
                                                            {
                                                                Ok(request) => {
                                                                    ws_srv.do_send(ObserverMessage{
                                                                        realm_name,
                                                                        body: ObserverMessageBody::<Request<G::GameRequest>, Update<G::GameUpdate >>::Request {
                                                                            player_id,
                                                                            request,
                                                                        }
                                                                    });
                                                                }
                                                                Err(err) => {
                                                                    warn!("deserialize binary err ignored {}", err);
                                                                }
                                                            }
                                                        }
                                                        Message::Text(_) => {
                                                            break PROTOCOL_ERROR;
                                                        }
                                                        Message::Ping(_) => {
                                                            // Axum spec says that automatic Pong will be sent.
                                                        }
                                                        Message::Pong(pong_data) => {
                                                            if rate_limiter.should_limit_rate_with_now(&RATE, last_activity) {
                                                                warn!("rate-limiting client pong");
                                                                continue;
                                                            }

                                                            if let Ok(bytes) = pong_data.try_into() {
                                                                let now = get_unix_time_now();
                                                                let timestamp = UnixTime::from_ne_bytes(bytes);
                                                                let rtt = now.saturating_sub(timestamp);
                                                                if rtt <= 10000 as UnixTime {
                                                                    ws_srv.do_send(ObserverMessage{
                                                                        realm_name,
                                                                        body: ObserverMessageBody::<Request<G::GameRequest>, Update<G::GameUpdate >>::RoundTripTime {
                                                                            player_id,
                                                                            rtt: rtt as u16,
                                                                        }
                                                                    });
                                                                }
                                                            } else {
                                                                warn!("received invalid pong data");
                                                            }
                                                        },
                                                        Message::Close(_) => {
                                                            info!("received close from client");
                                                            // tungstenite will echo close frame if necessary.
                                                            break SILENT_CLOSURE;
                                                        },
                                                    }
                                                }
                                                Err(error) => {
                                                    warn!("web socket error: {:?}", error);
                                                    break PROTOCOL_ERROR;
                                                }
                                            }
                                            None => {
                                                // web socket closed already.
                                                info!("web socket closed");
                                                break SILENT_CLOSURE;
                                            }
                                        }
                                    },
                                    maybe_observer_update = server_receiver.recv() => {
                                        let observer_update = match maybe_observer_update {
                                            Some(observer_update) => observer_update,
                                            None => {
                                                // infrastructure wants websocket closed.
                                                warn!("dropping web socket");
                                                break NORMAL_CLOSURE
                                            }
                                        };
                                        match observer_update {
                                            ObserverUpdate::Send{message} => {
                                                let bytes = core_protocol::bitcode::encode(&message).unwrap();
                                                let size = bytes.len();
                                                let web_socket_message = Message::Binary(bytes);
                                                if let Err(e) = web_socket.send(web_socket_message).await {
                                                    warn!("closing after failed to send {size} bytes: {e}");
                                                    break NORMAL_CLOSURE;
                                                }

                                                #[allow(clippy::collapsible_if)]
                                                if !measure_rtt_ping_governor.should_limit_rate_with_now(&MEASURE_RTT_PING, last_activity) {
                                                    if let Err(e) = web_socket.send(Message::Ping(get_unix_time_now().to_ne_bytes().into())).await {
                                                        warn!("closing after failed to ping: {e}");
                                                        break NORMAL_CLOSURE;
                                                    }
                                                }
                                            }
                                            ObserverUpdate::Close => {
                                                info!("closing web socket");
                                                break NORMAL_CLOSURE;
                                            }
                                        }
                                    },
                                    _ = keep_alive.as_mut() => {
                                        if last_activity.elapsed() < WEBSOCKET_HARD_TIMEOUT {
                                            if let Err(e) = web_socket.send(Message::Ping(get_unix_time_now().to_ne_bytes().into())).await {
                                                warn!("closing after failed to ping: {e}");
                                                break NORMAL_CLOSURE;
                                            }
                                            keep_alive.as_mut().reset((Instant::now() + TIMER_DURATION).into());
                                        } else {
                                            warn!("closing unresponsive");
                                            break PROTOCOL_ERROR;
                                        }
                                    }
                                }
                            };

                            ws_srv.do_send(ObserverMessage{
                                realm_name,
                                body: ObserverMessageBody::<Request<G::GameRequest>, Update<G::GameUpdate>>::Unregister {
                                    player_id,
                                    observer: server_sender,
                                }
                            });

                            if let Some(code) = closure {
                                let _ = web_socket.send(Message::Close(Some(CloseFrame{code, reason: "".into()}))).await;
                            } else {
                                let _ = web_socket.flush().await;
                            }
                        })),
                    },
                }
            }))
            .route("/system.json", axum::routing::get(move |ConnectInfo(addr): ConnectInfo<SocketAddr>, query: Query<SystemQuery>| {
                let srv = system_srv.to_owned();
                debug!("received system request");

                async move {
                    match srv
                        .send(SystemRequest {
                            server_number: query.server_number,
                            region_id: query.region_id.or_else(|| ip_to_region_id(addr.ip())),
                            invitation_id: query.invitation_id,
                        })
                        .await
                    {
                        Ok(system_response) => {
                            Ok(Json(system_response))
                        }
                        Err(e) => {
                            Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
                        }
                    }
                }
            }))
            .layer(axum::middleware::from_fn(async move |request: axum::http::Request<_>, next: axum::middleware::Next<_>| {
                let raw_path = request.uri().path();
                // The unwrap_or is purely defensive and should never happen.
                let path = raw_path.split('#').next().unwrap_or(raw_path);

                // We want to redirect everything except index.html (at any path level) so the
                // browser url-bar remains intact.
                let redirect = !path.is_empty() && !path.ends_with('/');

                if redirect {
                    let realm_name = request
                        .headers()
                        .get("host")
                        .and_then(|host|
                            host.to_str().ok())
                        .and_then(ExtractRealmName::parse);

                    if let Some(server_number) =
                            realm_name
                                .and_then(|realm_name|
                                    REALM_ROUTES
                                        .lock()
                                        .unwrap()
                                        .get(&realm_name)
                                        .copied()
                                        .filter(|&server_number| server_number != server_id.number || server_id.kind.is_local())
                                )
                                .or(ServerNumber::new(REDIRECT_TO_SERVER_ID.load(Ordering::Relaxed)))
                    {
                        let scheme = request.uri().scheme().cloned().unwrap_or(Scheme::HTTPS);
                        if let Ok(authority) = Authority::from_str(&format!("{}.{}", server_number.0.get(), G::GAME_ID.domain())) {
                            let mut builder =  Uri::builder()
                                .scheme(scheme)
                                .authority(authority);

                            if let Some(path_and_query) = request.uri().path_and_query() {
                                builder = builder.path_and_query(path_and_query.clone());
                            }

                            if let Ok(uri) = builder.build() {
                                return Err(Redirect::temporary(&uri.to_string()));
                            }
                        }
                    }
                }

                Ok(next.run(request).await)
            }))
            .route("/admin/", admin_router.clone())
            .route("/admin/*path", admin_router)
            .route("/plasma", axum::routing::post(move |_: Authenticated, update: Json<PlasmaUpdate>| {
                let srv = plasma_srv.to_owned();
                debug!("received plasma update");

                async move {
                    match srv.send(update.0).await {
                        Ok(plasma_response) => {
                            Ok(Json(plasma_response))
                        }
                        Err(e) => Err((StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response()),
                    }
                }
            }))
            .layer(ServiceBuilder::new()
                .layer(CorsLayer::new()
                    .allow_origin(tower_http::cors::AllowOrigin::predicate(move |origin, _parts| {
                        if cfg!(debug_assertions) {
                            true
                        } else {
                            let Ok(origin) = std::str::from_utf8(origin.as_bytes()) else {
                                return false;
                            };

                            let origin = origin
                                .trim_start_matches("http://")
                                .trim_start_matches("https://");

                            for domain in [G::GAME_ID.domain(), "localhost:8080", "localhost:8443", "localhost:80", "localhost:443", "softbear.com"] {
                                if let Some(prefix) = origin.strip_suffix(domain) {
                                    if prefix.is_empty() || prefix.ends_with('.') {
                                        return true;
                                    }
                                }
                            }

                            false
                        }
                    }))
                    .allow_headers(tower_http::cors::Any)
                    .allow_methods([Method::GET, Method::HEAD, Method::POST, Method::OPTIONS]))
                .layer(axum::middleware::from_fn(async move |request: axum::http::Request<_>, next: axum::middleware::Next<_>| {
                    let addr = request.extensions().get::<ConnectInfo<SocketAddr>>().map(|ci| ci.0);

                    if !request
                        .headers()
                        .get("auth")
                        .and_then(|hv| hv.to_str().ok())
                        .map(Authenticated::validate)
                        .unwrap_or(false) {
                        #[allow(clippy::question_mark)] // Breaks type inference on Ok.
                        if let Err(response) = limit_content_length(request.headers(), 16384) {
                            return Err(response);
                        }
                    }

                    let ip = addr.map(|addr| addr.ip());
                    let mut response = next.run(request).await;

                    // Add some universal default headers.
                    for (key, value) in [(CACHE_CONTROL, "no-cache")] {
                        if !response.headers().contains_key(key.clone()) {
                            response.headers_mut()
                                .insert(key, HeaderValue::from_static(value));
                        }
                    }

                    let content_length = response
                        .headers()
                        .get(axum::http::header::CONTENT_LENGTH)
                        .and_then(|h| h.to_str().ok())
                        .and_then(|s| u32::from_str(s).ok())
                        .unwrap_or(response.body().size_hint().lower() as u32)
                        .max(500);

                    if let Some(ip) = ip {
                        let should_rate_limit = {
                            HTTP_RATE_LIMITER
                                .lock()
                                .unwrap()
                                .should_limit_rate_with_usage(ip, content_length)
                        };

                        if should_rate_limit {
                            warn!("Bandwidth limiting {}", ip);

                            *response.status_mut() = StatusCode::TOO_MANY_REQUESTS;

                            // I changed my mind, I'm not actually going to send you all this data...
                            response = response.map(|_| {
                                boxed(Empty::new())
                            });
                        }
                    }

                    Ok(response)
                }))
            )
            // We limit even further later on.
            .layer(axum::extract::DefaultBodyLimit::max(64 * 1024 * 1024));

        let addr_incoming_config = axum_server::AddrIncomingConfig::new()
            .tcp_keepalive(Some(Duration::from_secs(32)))
            .tcp_nodelay(true)
            .tcp_sleep_on_accept_errors(true)
            .build();

        let http_config = axum_server::HttpConfig::new()
            .http1_keep_alive(true)
            .http1_header_read_timeout(Duration::from_secs(5))
            .max_buf_size(32768)
            .http2_max_concurrent_streams(Some(8))
            .http2_keep_alive_interval(Some(Duration::from_secs(4)))
            .http2_keep_alive_timeout(Duration::from_secs(10))
            //.http2_enable_connect_protocol()
            .http2_max_header_list_size(1024 * 1024)
            .http2_max_send_buf_size(65536)
            .http2_max_concurrent_streams(Some(64))
            .build();

        #[cfg(not(debug_assertions))]
        let http_app = Router::new()
            .fallback_service(get(async move |uri: Uri, host: TypedHeader<axum::headers::Host>, headers: reqwest::header::HeaderMap| {
                if let Err(response) = limit_content_length(&headers, 16384) {
                    return Err(response);
                }

                let mut parts = uri.into_parts();
                parts.scheme = Some(Scheme::HTTPS);
                let authority = if https_port == Options::STANDARD_HTTPS_PORT {
                    Authority::from_str(host.0.hostname())
                } else {
                    // non-standard port.
                    Authority::from_str(&format!("{}:{}", host.0.hostname(), https_port))
                }.map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())?;
                parts.authority = Some(authority);
                Uri::from_parts(parts)
                    .map(|uri| if http_port == Options::STANDARD_HTTP_PORT { Redirect::permanent(&uri.to_string()) } else { Redirect::temporary(&uri.to_string()) })
                    .map_err(|e| (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response())
            }));

        #[cfg(debug_assertions)]
        let http_app = app.clone();

        let http_server = axum_server::bind(SocketAddr::from(([0, 0, 0, 0], http_port)))
            .addr_incoming_config(addr_incoming_config.clone())
            .http_config(http_config.clone())
            .serve(http_app.into_make_service_with_connect_info::<SocketAddr>());

        let rustls_config = crate::net::tls::rustls_config(certificate_private_key_paths).await;

        let https_server = axum_server::bind_rustls(
            SocketAddr::from(([0, 0, 0, 0], https_port)),
            rustls_config
        )
            .addr_incoming_config(addr_incoming_config.clone())
            .http_config(http_config)
            .serve(app.into_make_service_with_connect_info::<SocketAddr>());

        tokio::select! {
            result = http_server => {
                error!("http server stopped: {:?}", result);
            }
            result = https_server => {
                error!("https server stopped: {:?}", result);
            }
            _ = tokio::signal::ctrl_c() => {
                error!("received Ctrl+C / SIGINT");
            }
        }

        srv.do_send(crate::shutdown::Shutdown);

        // Allow some time for the shutdown to propagate
        // but don't hang forever if it doesn't.
        tokio::time::sleep(Duration::from_secs(1)).await;
        std::process::exit(1);
    });
}
