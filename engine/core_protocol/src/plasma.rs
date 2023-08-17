// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::serde_util::is_default;
use crate::{
    id::{GameId, ServerId, SessionToken},
    metrics::{MetricFilter, Metrics},
    name::RealmName,
};
use crate::{
    ArenaToken, ClientHash, LeaderboardScoreDto, NickName, Notification, PeriodId, PlayerId,
    RealmDto, RegionId, ServerDto, ServerNumber, ServerToken, UnixTime, UserId,
};
use serde::{Deserialize, Serialize};
use std::net::Ipv4Addr;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaRequest {
    V1(PlasmaRequestV1),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaRequestV1 {
    /// Authenticate server prior to registration.
    ///
    /// # Cloud
    /// Plasma generates new key which authenticates future messages.
    ///
    /// # Local
    /// Key is assumed valid.
    Authenticate {
        game_id: GameId,
        /// # Cloud
        /// Plasma must send `ServerConfig` to the specified server using DNS lookup.
        server_id: ServerId,
    },
    /// Plasma should send pending updates in response to heartbeats from *local* servers.
    ///
    /// Sent every 60 seconds. Server considered dead if not received in last 180s.
    Heartbeat {
        game_id: GameId,
        server_id: ServerId,
        /// Number of real players online.
        #[serde(default, skip_serializing_if = "is_default")]
        player_count: u32,
        /// In case it changed.
        client_hash: ClientHash,
        /// For example, overutilized.
        #[serde(default, skip_serializing_if = "is_default")]
        unhealthy: bool,
        /// CPU utilization from 0 to 1.
        #[serde(default, skip_serializing_if = "is_default")]
        cpu: f32,
        /// RAM utilization from 0 to 1.
        #[serde(default, skip_serializing_if = "is_default")]
        ram: f32,
    },
    /// An arena has started. It is ignored if the server doesn't exist.
    ///
    /// If `realm_name` is `Some`, this registration expires after an hour,
    /// and will be resent before an hour elapses. Otherwise, registration
    /// does not expire.
    RegisterArena {
        game_id: GameId,
        server_id: ServerId,
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
        // For idempotency.
        arena_token: ArenaToken,
    },
    /// A session has started playing in an arena. It is ignored if the arena doesn't exist.
    /// This registration expires after an hour, and is resent before an hour elapses.
    RegisterPlayer {
        game_id: GameId,
        server_id: ServerId,
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
        session_token: SessionToken,
        /// For idempotency.
        arena_token: ArenaToken,
        /// Informational field.
        /// Latest player id, to replace any previous player id of this user in this arena.
        player_id: PlayerId,
    },
    /// Plasma should send [`Leaderboards`] and [`Servers`] in response.
    ///
    /// May be re-sent ocasionally, when fields change (notably `client_hash`).
    ///
    /// Registration does not expire.
    RegisterServer {
        game_id: GameId,
        server_id: ServerId,
        /// Informational field.
        #[serde(default, skip_serializing_if = "is_default")]
        region_id: Option<RegionId>,
        /// Public IPv4 address.
        #[serde(default, skip_serializing_if = "is_default")]
        ipv4_address: Option<Ipv4Addr>,
        client_hash: ClientHash,
    },
    RequestRealm {
        game_id: GameId,
        server_id: ServerId,
        realm_name: RealmName,
    },
    /// An arena has stopped. The arena and its players are cleared.
    UnregisterArena {
        game_id: GameId,
        server_id: ServerId,
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
        /// For idempotency.
        arena_token: ArenaToken,
    },
    /// A session has stopped playing in an arena. The player is cleared.
    UnregisterPlayer {
        game_id: GameId,
        server_id: ServerId,
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
        user_id: UserId,
        /// For idempotency.
        arena_token: ArenaToken,
        /// No longer valid for registration, but used for idempotency purposes.
        session_token: SessionToken,
    },
    /// A server has stopped. The server, its arenas, and their players are cleared.
    UnregisterServer {
        game_id: GameId,
        server_id: ServerId,
    },
    /// May be sent periodically for self-healing purposes.
    UpdateArenas {
        game_id: GameId,
        server_id: ServerId,
        realms: Box<[RealmDto]>,
    },
    UpdateLeaderboards {
        game_id: GameId,
        /// This server receives update via RPC whereas others receive a POST.
        ///
        /// Local servers and cloud servers have separate leaderboards.
        server_id: ServerId,
        /// Realms have separate leaderboards.
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
        scores: Box<[LeaderboardScoreDto]>,
    },
    /// Sent every hour to update metrics.
    UpdateMetrics {
        game_id: GameId,
        server_id: ServerId,
        timestamp: UnixTime,
        metrics: Box<[(Option<MetricFilter>, Metrics)]>,
    },
    /// May be sent periodically for self-healing purposes.
    UpdatePlayers {
        game_id: GameId,
        server_id: ServerId,
        realm_name: Option<RealmName>,
        players: Box<[UserId]>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[cfg_attr(feature = "server", derive(actix::Message))]
#[cfg_attr(feature = "server", rtype(result = "()"))]
pub enum PlasmaUpdate {
    /// Version 1 protocol.
    V1(Box<[PlasmaUpdateV1]>),
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum PlasmaUpdateV1 {
    /// Sent after [`RegisterArena`] where [`realm_name`] is [`Some`] and when updated.
    ConfigArena {
        realm_name: RealmName,
        /// For idempotency.
        arena_token: ArenaToken,
        /// If true, each player must await [`ConfigPlayer`] before play begins.
        #[serde(default, skip_serializing_if = "is_default")]
        private: bool,
    },
    /// Sent after [`RegisterPlayer`] and when updated (for players with user ids).
    ConfigPlayer {
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
        user_id: UserId,
        /// For idempotency.
        arena_token: ArenaToken,
        /// For idempotency.
        session_token: SessionToken,
        /// Id of player, for efficient lookup.
        player_id: PlayerId,
        /// In-game admin priviledges.
        #[serde(default, skip_serializing_if = "is_default")]
        admin: bool,
        /// True if user is NOT allowed to play in arena/realm.
        #[serde(default, skip_serializing_if = "is_default")]
        ban: bool,
        /// In-game moderator priviledges.
        #[serde(default, skip_serializing_if = "is_default")]
        moderator: bool,
        /// Unique nick name.
        #[serde(default, skip_serializing_if = "is_default")]
        nick_name: Option<NickName>,
    },
    /// [`Heartbeat`] (self-healing), and when updated.
    ConfigServer {
        /// If `Some`, the new/correct key.
        #[serde(default, skip_serializing_if = "is_default")]
        token: Option<ServerToken>,
        /// If `Some`, the new/correct role.
        role: Option<ServerRole>,
    },
    /// Sent in response to [`RegisterServer`].
    ///
    /// Also sent to communicate how leaderboard(s) changed e.g. due
    /// to [`UpdateLeaderboards`] from any game server.
    ///
    /// Deprecated in favor of Leaderboard (soon).
    Leaderboards {
        leaderboards: Box<[(PeriodId, Box<[LeaderboardScoreDto]>)]>,
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
    },
    /// Sent in response to [`RegisterServer`].
    ///
    /// Also sent to communicate how leaderboard changed e.g. due
    /// to [`UpdateLeaderboards`] from any game server.
    Leaderboard {
        #[serde(default, skip_serializing_if = "is_default")]
        realm_name: Option<RealmName>,
        period_id: PeriodId,
        scores: Box<[LeaderboardScoreDto]>,
    },
    Notification {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        realm_name: Option<RealmName>,
        /// Id of player.
        player_id: PlayerId,
        /// For idempotency.
        arena_token: ArenaToken,
        /// For idempotency.
        session_token: SessionToken,
        /// Id of user (sanity check to ensure arena_id and player_id are up to date).
        user_id: UserId,
        /// Notification content.
        notification: Notification,
    },
    /// Sent after [`RegisterServer`], [`RequestRealm`] and when updated.
    /// If disabled is true then de-allocate the realm.  If server number
    /// is provided, then redirect is required.
    Realms {
        added: Box<[RealmDto]>,
        removed: Box<[RealmName]>,
    },
    /// Sent in response to [`Authenticate`] (initialization),
    /// All servers satisfying the following conditions:
    /// - Cloud (not local)
    /// - [`RegionId`] is known
    /// - Client hash is compatible
    /// - Active role
    /// - Healthy
    /// - Recent enough heartbeat
    ///
    /// Exception: Server receives this message, it should be included regardless
    /// of role, health, or heartbeat. Client hash is trivially compatible. All
    /// other conditions still apply.
    ///
    /// Sent in response to [`RegisterServer`] and either [`Heartbeat`] (self-healing)
    /// OR when updated by the heartbeat of another server
    /// (e.g. player count change), the latter of which is more frequent when there
    /// are more servers.
    Servers { servers: Box<[ServerDto]> },
}

#[derive(Copy, Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub enum ServerRole {
    /// - Accepts new connections
    /// - Hidden from server selector
    /// - Possibly unhealthy
    /// - Never redirects to other servers
    /// - Servers reset to this state if they don't hear from plasma
    #[default]
    Unlisted,
    /// - Redirects all new connections to specified (presumably active) server (admins should
    ///   never need to specify *which* active server to redirect to)
    /// - Hidden from server selector
    /// - Possibly unhealthy
    /// - May be promoted to `Active` during failover if healthy, irrespective of client hash
    Redirected(ServerNumber),
    /// - Accepts new connections
    /// - Optionally advertises a more optimal active server, depending on the player
    /// - Displayed in server selector when client hash is compatible
    /// - Never redirects to other servers
    /// - Is (was recently) healthy
    Public,
}

impl ServerRole {
    pub fn is_unlisted(self) -> bool {
        matches!(self, Self::Unlisted)
    }

    pub fn is_redirected(self) -> bool {
        matches!(self, Self::Redirected(_))
    }

    pub fn redirect(self) -> Option<ServerNumber> {
        if let Self::Redirected(redirect) = self {
            Some(redirect)
        } else {
            None
        }
    }

    pub fn is_public(self) -> bool {
        matches!(self, Self::Public)
    }
}
