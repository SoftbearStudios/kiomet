// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::apply::Apply;
use crate::browser_storage::BrowserStorages;
use crate::frontend::Frontend;
use crate::game_client::GameClient;
use crate::js_util::{host, invitation_id, is_https, ws_protocol};
use crate::keyboard::KeyboardState;
use crate::mouse::MouseState;
use crate::reconn_web_socket::ReconnWebSocket;
use crate::setting::CommonSettings;
use crate::visibility::VisibilityState;
use core_protocol::dto::{
    LeaderboardScoreDto, LiveboardDto, MessageDto, PlayerDto, ServerDto, TeamDto, YourScoreDto,
};
use core_protocol::id::{CohortId, InvitationId, PeriodId, PlayerId, TeamId};
use core_protocol::name::PlayerAlias;
use core_protocol::owned::{dedup_into_inner, owned_into_box, owned_into_iter};
use core_protocol::rpc::{
    ChatUpdate, ClientRequest, ClientUpdate, InvitationUpdate, LeaderboardUpdate, LiveboardUpdate,
    PlayerUpdate, Request, SystemUpdate, TeamUpdate, Update, WebSocketQuery,
};
use core_protocol::ServerNumber;
use heapless::HistoryBuffer;
use std::collections::{BTreeMap, HashMap};
use std::marker::PhantomData;
use std::rc::Rc;

#[cfg(feature = "audio")]
use crate::audio::AudioPlayer;

/// The context (except rendering) of a game.
pub struct Context<G: GameClient + ?Sized> {
    /// General client state.
    pub client: ClientState,
    /// Server state
    pub state: ServerState<G>,
    /// Server websocket
    pub socket: ReconnWebSocket<Update<G::GameUpdate>, Request<G::GameRequest>, ServerState<G>>,
    /// Audio player (volume managed automatically).
    #[cfg(feature = "audio")]
    pub audio: AudioPlayer<G::Audio>,
    /// Keyboard input.
    pub keyboard: KeyboardState,
    /// Mouse input.
    pub mouse: MouseState,
    /// Whether the page is visible.
    pub visibility: VisibilityState,
    /// Settings.
    pub settings: G::GameSettings,
    /// Common settings.
    pub common_settings: CommonSettings,
    /// Local storage.
    pub browser_storages: BrowserStorages,
    pub(crate) frontend: Box<dyn Frontend<G::UiProps> + 'static>,
}

/// State common to all clients.
#[derive(Default)]
pub struct ClientState {
    /// Time of last or current update.
    pub time_seconds: f32,
    /// Supports rewarded ads.
    pub rewarded_ads: bool,
}

/// Obtained from server via websocket.
pub struct ServerState<G: GameClient> {
    pub game: G::GameState,
    pub core: Rc<CoreState>,
}

/// Server state specific to core functions
#[derive(Default)]
pub struct CoreState {
    pub cohort_id: Option<CohortId>,
    pub player_id: Option<PlayerId>,
    pub created_invitation_id: Option<InvitationId>,
    /// Ordered, i.e. first is captain.
    pub members: Box<[PlayerId]>,
    pub joiners: Box<[PlayerId]>,
    pub joins: Box<[TeamId]>,
    pub leaderboards: [Box<[LeaderboardScoreDto]>; std::mem::variant_count::<PeriodId>()],
    pub liveboard: Vec<LiveboardDto>,
    pub messages: HistoryBuffer<MessageDto, 9>,
    pub(crate) players: HashMap<PlayerId, PlayerDto>,
    pub real_players: u32,
    pub teams: HashMap<TeamId, TeamDto>,
    pub servers: BTreeMap<ServerNumber, ServerDto>,
    pub your_score: Option<YourScoreDto>,
}

impl<G: GameClient> Default for ServerState<G> {
    fn default() -> Self {
        Self {
            game: G::GameState::default(),
            core: Default::default(),
        }
    }
}

impl CoreState {
    /// Gets whether a player is friendly to an other player, taking into account team membership.
    /// Returns false if `other_player_id` is None.
    pub fn is_friendly(&self, other_player_id: Option<PlayerId>) -> bool {
        self.are_friendly(self.player_id, other_player_id)
    }

    /// Gets whether player is friendly to other player, taking into account team membership.
    /// Returns false if either `PlayerId` is None.
    pub fn are_friendly(
        &self,
        player_id: Option<PlayerId>,
        other_player_id: Option<PlayerId>,
    ) -> bool {
        player_id
            .zip(other_player_id)
            .map(|(id1, id2)| {
                id1 == id2
                    || self
                        .team_id_lookup(id1)
                        .zip(self.team_id_lookup(id2))
                        .map(|(id1, id2)| id1 == id2)
                        .unwrap_or(false)
            })
            .unwrap_or(false)
    }

    /// Gets player's `PlayerDto`.
    pub fn player(&self) -> Option<&PlayerDto> {
        self.player_id.and_then(|id| self.players.get(&id))
    }

    /// Player or bot (simulated) `PlayerDto`.
    pub fn player_or_bot(&self, player_id: PlayerId) -> Option<PlayerDto> {
        player_id
            .is_bot()
            .then(|| {
                Some(PlayerDto {
                    alias: PlayerAlias::from_bot_player_id(player_id),
                    player_id,
                    team_captain: false,
                    admin: false,
                    moderator: false,
                    team_id: None,
                    user_id: None,
                    authentic: false,
                })
            })
            .unwrap_or_else(|| self.players.get(&player_id).cloned())
    }

    /// Gets hashmap that contains players, but *not* bots.
    pub fn only_players(&self) -> &HashMap<PlayerId, PlayerDto> {
        &self.players
    }

    /// Gets player's team's `TeamDto`.
    pub fn team(&self) -> Option<&TeamDto> {
        self.team_id().and_then(|id| self.teams.get(&id))
    }

    /// Gets player's `TeamId`.
    pub fn team_id(&self) -> Option<TeamId> {
        self.player_id.and_then(|id| self.team_id_lookup(id))
    }

    /// Gets a player's `TeamId`.
    fn team_id_lookup(&self, player_id: PlayerId) -> Option<TeamId> {
        self.players.get(&player_id).and_then(|p| p.team_id)
    }

    pub fn leaderboard(&self, period_id: PeriodId) -> &[LeaderboardScoreDto] {
        &self.leaderboards[period_id as usize]
    }
}

impl<G: GameClient> Apply<Update<G::GameUpdate>> for ServerState<G> {
    fn apply(&mut self, update: Update<G::GameUpdate>) {
        // Use rc_borrow_mut to keep semantics of shared references the same while sharing with
        // yew_frontend.
        use rc_borrow_mut::RcBorrowMut;
        let mut core = Rc::borrow_mut(&mut self.core);

        match update {
            Update::Chat(update) => {
                if let ChatUpdate::Received(received) = update {
                    // Need to use into_vec since
                    // https://github.com/rust-lang/rust/issues/59878 is incomplete.
                    core.messages
                        .extend(received.into_vec().into_iter().map(dedup_into_inner));
                }
            }
            Update::Client(update) => {
                if let ClientUpdate::SessionCreated {
                    cohort_id,
                    player_id,
                    ..
                } = update
                {
                    core.cohort_id = Some(cohort_id);
                    core.player_id = Some(player_id);
                }
            }
            Update::Game(update) => {
                self.game.apply(update);
            }
            Update::Invitation(update) => match update {
                InvitationUpdate::Accepted => {}
                InvitationUpdate::Created(invitation_id) => {
                    core.created_invitation_id = Some(invitation_id);
                }
            },
            Update::Leaderboard(update) => match update {
                LeaderboardUpdate::Updated(period_id, leaderboard) => {
                    core.leaderboards[period_id as usize] = owned_into_box(leaderboard);
                }
            },
            Update::Liveboard(LiveboardUpdate::Updated {
                added,
                removed,
                your_score,
            }) => {
                let liveboard = &mut core.liveboard;

                // Remove items that were removed or will be added.
                liveboard.retain(|i| {
                    !(removed.contains(&i.player_id)
                        || added.iter().any(|a| a.player_id == i.player_id))
                });

                // Only inserting in sorted order, not updating in place.
                // Invariant added cannot contain duplicate player ids.
                for item in owned_into_iter(added) {
                    // unwrap_err will never panic because player ids are unique because
                    // we searched for them with find.
                    let index = liveboard
                        .binary_search_by(|other| {
                            // Put higher scores higher on leaderboard.
                            // If scores are equal, ensure total ordering with player id.
                            // NOTE: order of cmp is reversed compared to sort_by.
                            item.score
                                .cmp(&other.score)
                                .then_with(|| other.player_id.cmp(&item.player_id))
                        })
                        .inspect(|_| debug_assert!(false))
                        .unwrap_or_else(|i| i);

                    // Only inserting in correct position to maintain sorted order.
                    liveboard.insert(index, item.clone());
                }

                if your_score.is_some() {
                    core.your_score = your_score;
                }
            }
            Update::Player(update) => {
                if let PlayerUpdate::Updated {
                    added,
                    removed,
                    real_players,
                } = update
                {
                    for player in owned_into_iter(added) {
                        core.players.insert(player.player_id, player);
                    }
                    for player_id in removed.iter() {
                        core.players.remove(player_id);
                    }
                    core.real_players = real_players;
                }
            }
            Update::System(update) => match update {
                SystemUpdate::Added(added) => {
                    for server in owned_into_iter(added) {
                        core.servers.insert(server.server_number, server);
                    }
                }
                SystemUpdate::Removed(removed) => {
                    for server_number in removed.iter() {
                        core.servers.remove(server_number);
                    }
                }
            },
            Update::Team(update) => match update {
                TeamUpdate::Members(members) => {
                    core.members = owned_into_box(members);
                }
                TeamUpdate::Joiners(joiners) => {
                    core.joiners = joiners;
                }
                TeamUpdate::Joins(joins) => {
                    core.joins = joins;
                }
                TeamUpdate::AddedOrUpdated(added_or_updated) => {
                    for team in owned_into_iter(added_or_updated) {
                        core.teams.insert(team.team_id, team);
                    }
                }
                TeamUpdate::Removed(removed) => {
                    for team_id in removed.iter() {
                        core.teams.remove(team_id);
                    }
                }
                _ => {}
            },
        }
    }
}

impl<G: GameClient> Context<G> {
    pub(crate) fn new(
        mut browser_storages: BrowserStorages,
        mut common_settings: CommonSettings,
        settings: G::GameSettings,
        frontend: Box<dyn Frontend<G::UiProps> + 'static>,
    ) -> Self {
        let server_number = frontend.get_ideal_server_number();
        let host = Self::compute_websocket_host(&common_settings, server_number, &*frontend);
        let socket = ReconnWebSocket::new(host, None);
        common_settings.set_server_number(server_number, &mut browser_storages);

        Self {
            #[cfg(feature = "audio")]
            audio: AudioPlayer::default(),
            client: ClientState::default(),
            state: ServerState::default(),
            socket,
            keyboard: KeyboardState::default(),
            mouse: MouseState::default(),
            visibility: VisibilityState::default(),
            settings,
            common_settings,
            browser_storages,
            frontend,
        }
    }

    pub(crate) fn compute_websocket_host(
        common_settings: &CommonSettings,
        ideal_server_number: Option<ServerNumber>,
        frontend: &dyn Frontend<G::UiProps>,
    ) -> String {
        let (encryption, host) = ideal_server_number
            //.filter(|_| !host.starts_with("localhost"))
            .map(|id: ServerNumber| (true, format!("{}.{}", id.0, G::GAME_ID.domain())))
            .unwrap_or_else(|| {
                (
                    frontend.get_real_encryption().unwrap_or(is_https()),
                    frontend.get_real_host().unwrap_or_else(host),
                )
            });

        // crate::console_log!("override={:?} ideal server={:?}, host={:?}, ideal_host={:?}", override_server_id, ideal_server_id, host, ideal_host);

        let web_socket_query = WebSocketQuery {
            player_id: common_settings.player_id,
            token: common_settings.token,
            session_token: common_settings.session_token,
            invitation_id: invitation_id(),
            date_created: common_settings.date_created,
            cohort_id: common_settings.cohort_id,
            referrer: frontend.get_real_referrer(),
        };

        // TODO to_string should take &impl Serialize.
        let web_socket_query_url = serde_urlencoded::to_string(web_socket_query).unwrap();

        format!(
            "{}://{}/ws?{}",
            ws_protocol(encryption),
            host,
            web_socket_query_url
        )
    }

    /// Shorter version of `context.state.core.player_id`.
    pub fn player_id(&self) -> Option<PlayerId> {
        self.state.core.player_id
    }

    /// Whether the game websocket is closed or errored (not open, opening, or nonexistent).
    pub fn connection_lost(&self) -> bool {
        self.socket.is_terminated()
    }

    /// Send a game command on the socket.
    pub fn send_to_game(&mut self, request: G::GameRequest) {
        self.send_to_server(Request::Game(request));
    }

    /// Send a request to set the player's alias.
    pub fn send_set_alias(&mut self, alias: PlayerAlias) {
        self.send_to_server(Request::Client(ClientRequest::SetAlias(alias)));
    }

    /// Send a request to log an error message.
    pub fn send_trace(&mut self, message: String) {
        self.send_to_server(Request::Client(ClientRequest::Trace { message }));
    }

    /// Send a request on the socket.
    pub fn send_to_server(&mut self, request: Request<G::GameRequest>) {
        self.socket.send(request);
    }

    /// Set the props used to render the UI. Javascript must implement part of this.
    pub fn set_ui_props(&mut self, props: G::UiProps) {
        self.frontend.set_ui_props(props);
    }

    /// Enable visibility cheating.
    pub fn cheats(&self) -> bool {
        cfg!(debug_assertions)
    }
}

#[derive(Clone)]
pub struct WeakCoreState(std::rc::Weak<CoreState>);

impl Default for WeakCoreState {
    fn default() -> Self {
        thread_local! {
            static DEFAULT_CORE_STATE: Rc<CoreState> = Rc::default();
        }
        DEFAULT_CORE_STATE.with(Self::new) // Only allocate zero value once to not cause a leak.
    }
}

impl PartialEq for WeakCoreState {
    fn eq(&self, _other: &Self) -> bool {
        // std::ptr::eq(self, _other)
        false // Can't implement Eq because not reflexive but probably doesn't matter...
    }
}

impl WeakCoreState {
    /// Borrow the core state immutably. Unused for now.
    pub fn as_strong(&self) -> StrongCoreState {
        StrongCoreState {
            inner: self.0.upgrade().unwrap(),
            _spooky: PhantomData,
        }
    }

    /// Like [`Self::as_strong`] but consumes self and has a static lifetime.
    pub fn into_strong(self) -> StrongCoreState<'static> {
        StrongCoreState {
            inner: self.0.upgrade().unwrap(),
            _spooky: PhantomData,
        }
    }

    /// Create a [`WeakCoreState`] from a [`Rc<CoreState>`].
    pub fn new(core: &Rc<CoreState>) -> Self {
        Self(Rc::downgrade(core))
    }
}

pub struct StrongCoreState<'a> {
    inner: Rc<CoreState>,
    _spooky: PhantomData<&'a ()>,
}

impl<'a> std::ops::Deref for StrongCoreState<'a> {
    type Target = CoreState;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}
