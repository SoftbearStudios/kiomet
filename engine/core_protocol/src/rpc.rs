// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::dto::*;
use crate::id::*;
use crate::name::*;
use crate::owned::{Dedup, Owned};
use crate::UnixTime;
use bitcode::{Decode, Encode};
use serde::{Deserialize, Serialize};

/// See https://docs.rs/actix/latest/actix/dev/trait.MessageResponse.html
macro_rules! actix_response {
    ($typ: ty) => {
        #[cfg(feature = "server")]
        impl<A, M> actix::dev::MessageResponse<A, M> for $typ
        where
            A: actix::Actor,
            M: actix::Message<Result = $typ>,
        {
            fn handle(
                self,
                _ctx: &mut A::Context,
                tx: Option<actix::dev::OneshotSender<M::Result>>,
            ) {
                if let Some(tx) = tx {
                    let _ = tx.send(self);
                }
            }
        }
    };
}

/// Pass the following query parameters to the system endpoint to inform server routing.
#[derive(Debug, Serialize, Deserialize)]
pub struct SystemQuery {
    /// Express a [`ServerNumber`] preference. `None` means localhost/unknown.
    /// It is not guaranteed to be honored.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub server_number: Option<ServerNumber>,
    /// Express a region preference. It is not guaranteed to be honored.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub region_id: Option<RegionId>,
    /// Express a preference in being placed with the inviting player. It is not guaranteed to be honored.
    #[serde(default)]
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invitation_id: Option<InvitationId>,
}

/// Response to system request.
#[derive(Serialize, Deserialize)]
#[serde(rename = "camelCase")]
pub struct SystemResponse {
    /// The [`ServerNumber`] matching the invitation, or closest to the client.
    /// [`None`] means connect to the same host.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_number: Option<ServerNumber>,
}

actix_response!(SystemResponse);

/// Initiate a websocket with these optional parameters in the URL query string.
#[derive(Debug, Serialize, Deserialize)]
pub struct WebSocketQuery {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub player_id: Option<PlayerId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub token: Option<Token>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub session_token: Option<SessionToken>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invitation_id: Option<InvitationId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub referrer: Option<Referrer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub cohort_id: Option<CohortId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub date_created: Option<UnixTime>,
}

/// Client to server request.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum Request<GR> {
    Chat(ChatRequest),
    Client(ClientRequest),
    Game(GR),
    Invitation(InvitationRequest),
    Player(PlayerRequest),
    Team(TeamRequest),
}

#[cfg(feature = "server")]
impl<GR: Serialize + serde::de::DeserializeOwned + actix::Message> actix::Message for Request<GR>
where
    <GR as actix::Message>::Result: Serialize + serde::de::DeserializeOwned,
{
    type Result = Update<GR::Result>;
}

/// Server to client update.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
#[cfg_attr(feature = "server", derive(actix::Message))]
#[cfg_attr(feature = "server", rtype(result = "()"))]
pub enum Update<GU> {
    Chat(ChatUpdate),
    Client(ClientUpdate),
    Game(GU),
    Invitation(InvitationUpdate),
    Leaderboard(LeaderboardUpdate),
    Liveboard(LiveboardUpdate),
    Player(PlayerUpdate),
    System(SystemUpdate),
    Team(TeamUpdate),
}

/// Team related requests from the client to the server.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum TeamRequest {
    Accept(PlayerId),
    Create(TeamName),
    Join(TeamId),
    Kick(PlayerId),
    Leave,
    Promote(PlayerId),
    Reject(PlayerId),
}

/// Team related update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum TeamUpdate {
    Accepted(PlayerId),
    AddedOrUpdated(Owned<[TeamDto]>),
    Created(TeamId, TeamName),
    /// A complete enumeration of joiners, for the team captain only.
    Joiners(Box<[PlayerId]>),
    Joining(TeamId),
    /// The following is for the joiner only, to indicate which teams they are joining.
    Joins(Box<[TeamId]>),
    Kicked(PlayerId),
    Left,
    /// A complete enumeration of team members, in order (first is captain).
    Members(Owned<[PlayerId]>),
    Promoted(PlayerId),
    Rejected(PlayerId),
    Removed(Owned<[TeamId]>),
}

/// Chat related request from client to server.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum ChatRequest {
    /// Avoid seeing this player's messages.
    Mute(PlayerId),
    /// For moderators only.
    RestrictPlayer { player_id: PlayerId, minutes: u32 },
    /// Send a chat message.
    Send {
        message: String,
        /// Whether messages should only be visible to sender's team.
        whisper: bool,
    },
    /// Chat will be in safe mode for this many more minutes. For moderators only.
    SetSafeMode(u32),
    /// Chat will be in slow mode for this many more minutes. For moderators only.
    SetSlowMode(u32),
    /// Resume seeing this player's messages.
    Unmute(PlayerId),
}

/// Chat related update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum ChatUpdate {
    Muted(PlayerId),
    PlayerRestricted { player_id: PlayerId, minutes: u32 },
    Received(Box<[Dedup<MessageDto>]>),
    SafeModeSet(u32),
    SlowModeSet(u32),
    Sent,
    Unmuted(PlayerId),
}

/// Player related request from client to server.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum PlayerRequest {
    Report(PlayerId),
}

/// Player related update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum PlayerUpdate {
    Reported(PlayerId),
    Updated {
        added: Owned<[PlayerDto]>,
        removed: Owned<[PlayerId]>,
        real_players: u32,
    },
}

/// Leaderboard related update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum LeaderboardUpdate {
    // The leaderboard contains high score players, but not teams, for prior periods.
    Updated(PeriodId, Owned<[LeaderboardScoreDto]>),
}

/// Liveboard related update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum LiveboardUpdate {
    // The liveboard contains high score players and their teams in the current game.
    Updated {
        added: Owned<[LiveboardDto]>,
        removed: Owned<[PlayerId]>,
        your_score: Option<YourScoreDto>,
    },
}

/// Invitation related request from client to server.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum InvitationRequest {
    Create,
    Accept(InvitationId),
}

/// Invitation related update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum InvitationUpdate {
    Created(InvitationId),
    Accepted,
}

/// General request from client to server.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum ClientRequest {
    /// Present a Plasma session id.
    Login(SessionToken),
    SetAlias(PlayerAlias),
    /// An advertisement was shown or played.
    TallyAd(AdType),
    TallyFps(f32),
    Trace {
        message: String,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum AdType {
    Banner,
    Rewarded,
    Video,
}

/// General update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum ClientUpdate {
    AdTallied,
    AliasSet(PlayerAlias),
    EvalSnippet(Owned<str>),
    FpsTallied,
    LoggedIn(SessionToken),
    SessionCreated {
        cohort_id: CohortId,
        server_number: Option<ServerNumber>,
        realm_name: Option<RealmName>,
        player_id: PlayerId,
        token: Token,
        date_created: UnixTime,
    },
    Traced,
}

/// General update from server to client.
#[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
pub enum SystemUpdate {
    Added(Owned<[ServerDto]>),
    Removed(Owned<[ServerNumber]>),
}

#[cfg(feature = "admin")]
pub use admin::*;
#[cfg(feature = "admin")]
mod admin {
    use super::*;
    use crate::metrics::MetricFilter;

    /// Admin requests are from the admin interface to the core service.
    #[derive(Clone, Debug, Deserialize, Serialize)]
    #[cfg_attr(feature = "server", derive(actix::Message))]
    #[cfg_attr(
        feature = "server",
        rtype(result = "Result<AdminUpdate, &'static str>")
    )]
    pub enum AdminRequest {
        ClearSnippet {
            snippet_id: SnippetId,
        },
        MutePlayer {
            player_id: PlayerId,
            minutes: usize,
        },
        OverridePlayerAlias {
            player_id: PlayerId,
            alias: PlayerAlias,
        },
        OverridePlayerModerator {
            player_id: PlayerId,
            moderator: bool,
        },
        RequestDay {
            filter: Option<MetricFilter>,
        },
        RequestGames,
        RequestPlayers,
        RequestProfile,
        RequestReferrers,
        RequestRegions,
        RequestSeries {
            game_id: GameId,
            server_id: Option<ServerId>,
            filter: Option<MetricFilter>,
            period_start: Option<UnixTime>,
            period_stop: Option<UnixTime>,
            // Resolution in hours.
            resolution: Option<std::num::NonZeroU8>,
        },
        /// Qualifies the result of RequestDay and RequestSummary.
        RequestServerId,
        RequestSnippets,
        RequestSummary {
            filter: Option<MetricFilter>,
        },
        RequestUserAgents,
        RestrictPlayer {
            player_id: PlayerId,
            minutes: usize,
        },
        SendChat {
            // If None, goes to all players.
            player_id: Option<PlayerId>,
            alias: PlayerAlias,
            message: String,
        },
        SetGameClient(minicdn::EmbeddedMiniCdn),
        SetRustrictTrie(rustrict::Trie),
        SetRustrictReplacements(rustrict::Replacements),
        SetSnippet {
            #[serde(flatten)]
            snippet_id: SnippetId,
            snippet: Owned<str>,
        },
    }

    /// Admin related responses from the server.
    #[derive(Clone, Debug, Serialize)]
    pub enum AdminUpdate {
        ChatSent,
        DayRequested(Owned<[(UnixTime, MetricsDataPointDto)]>),
        GameClientSet(ClientHash),
        RustrictTrieSet,
        RustrictReplacementsSet,
        GamesRequested(Box<[(GameId, f32)]>),
        HttpServerRestarting,
        PlayerAliasOverridden(PlayerAlias),
        PlayerModeratorOverridden(bool),
        PlayerMuted(usize),
        PlayerRestricted(usize),
        PlayersRequested(Box<[AdminPlayerDto]>),
        ProfileRequested(String),
        RedirectRequested(Option<ServerNumber>),
        RedirectSet(Option<ServerNumber>),
        ReferrersRequested(Box<[(Referrer, f32)]>),
        RegionsRequested(Box<[(RegionId, f32)]>),
        SeriesRequested(Owned<[(UnixTime, MetricsDataPointDto)]>),
        ServerIdRequested(ServerId),
        SnippetCleared,
        SnippetSet,
        SnippetsRequested(Box<[SnippetDto]>),
        SummaryRequested(Box<MetricsSummaryDto>),
        UserAgentsRequested(Box<[(UserAgentId, f32)]>),
    }
}
