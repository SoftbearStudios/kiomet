// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::world::Apply;
use fxhash::FxHashSet;
use kodiak_common::actor_model::{Actor, Message};
use kodiak_common::bitcode::{self, *};
use kodiak_common::{Hashable, PlayerId};

#[derive(Clone, Debug, Default, Hash, Encode, Decode)]
pub struct Player {
    pub allies: Hashable<FxHashSet<PlayerId>>, // TODO better set/map.
    pub new_alliances: Hashable<FxHashSet<PlayerId>>,
}

impl Actor for Player {
    type Id = PlayerId;
    /// Players are based on chunks which already have a keepalive.
    /// However, must be at least 1 so the player associated with
    /// vanishing forces is preserved long enough for clients
    /// to simulate the tick.
    const KEEPALIVE: u8 = 1;
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum PlayerInput {
    Died,
    /// Single direction alliance request.
    AddAlly(PlayerId),
    /// Bidirectional alliance formed this tick.
    NewAlliance(PlayerId),
    /// Cancel signle direction alliance request.
    RemoveAlly(PlayerId),
}

impl Message for PlayerInput {}

impl<C> Apply<PlayerInput, C> for Player {
    fn apply(&mut self, u: &PlayerInput, _: &mut C) {
        match u.clone() {
            PlayerInput::Died => {
                self.allies.clear();
                //self.new_alliances.clear();
            }
            PlayerInput::AddAlly(player_id) => {
                let _inserted = self.allies.insert(player_id);
                //debug_assert!(_inserted);
            }
            PlayerInput::NewAlliance(player_id) => {
                self.new_alliances.insert(player_id);
            }
            PlayerInput::RemoveAlly(player_id) => {
                let _removed = self.allies.remove(&player_id);
                //debug_assert!(_removed);
            }
        }
    }
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum PlayerMaintainance {
    Died,
    RemoveDeadAlly(PlayerId),
}

impl Message for PlayerMaintainance {}

impl<C> Apply<PlayerMaintainance, C> for Player {
    fn apply(&mut self, u: &PlayerMaintainance, _: &mut C) {
        match u.clone() {
            PlayerMaintainance::Died => {
                self.allies.clear();
            }
            PlayerMaintainance::RemoveDeadAlly(player_id) => {
                let _removed = self.allies.remove(&player_id);
                //debug_assert!(_removed);
            }
        }
    }
}
