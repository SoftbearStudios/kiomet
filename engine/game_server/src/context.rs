// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::bot::BotRepo;
use crate::chat::ChatRepo;
use crate::game_service::GameArenaService;
use crate::liveboard::LiveboardRepo;
use crate::player::PlayerRepo;
#[cfg(feature = "teams")]
use crate::team::TeamRepo;
use core_protocol::ArenaToken;
use rand::{thread_rng, Rng};

/// Things that go along with every instance of a [`GameArenaService`].
pub struct Context<G: GameArenaService> {
    pub token: ArenaToken,
    pub players: PlayerRepo<G>,
    pub(crate) bots: BotRepo<G>,
    pub(crate) chat: ChatRepo<G>,
    #[cfg(feature = "teams")]
    pub teams: TeamRepo<G>,
    pub(crate) liveboard: LiveboardRepo<G>,
}

impl<G: GameArenaService> Context<G> {
    pub fn new(bots: BotRepo<G>, chat_log: Option<String>) -> Self {
        Context {
            token: ArenaToken(thread_rng().gen()),
            bots,
            players: PlayerRepo::default(),
            #[cfg(feature = "teams")]
            teams: TeamRepo::default(),
            chat: ChatRepo::new(chat_log),
            liveboard: LiveboardRepo::default(),
        }
    }
}
