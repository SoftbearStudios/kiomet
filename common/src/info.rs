// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::tower::TowerId;
use crate::unit::Unit;
use kodiak_common::glam::Vec2;
use kodiak_common::PlayerId;

pub type OnInfo<'a> = dyn FnMut(InfoEvent) + 'a;

//pub trait OnInfo {
//    fn on_info(&mut self, info: InfoEvent);
//}

//impl<T: FnMut(InfoEvent)> OnInfo for T {
//    fn on_info(&mut self, info: InfoEvent) {
//        self(info)
//    }
//}

#[derive(Debug, Copy, Clone)]
pub struct InfoEvent {
    pub position: Vec2,
    pub info: Info,
}

#[derive(Debug, Copy, Clone)]
pub enum Info {
    GainedTower {
        tower_id: TowerId,
        player_id: PlayerId,
        reason: GainedTowerReason,
    },
    LostForce(PlayerId),
    /// Isn't fired when player is killed.
    LostRuler {
        player_id: PlayerId,
        reason: LostRulerReason,
    },
    LostTower {
        tower_id: TowerId,
        /// Player who lost the tower.
        player_id: PlayerId,
        reason: LostTowerReason,
    },
    Emp(Option<PlayerId>),
    NuclearExplosion,
    ShellExplosion,
}

#[derive(Copy, Clone, Debug)]
pub enum LostRulerReason {
    KilledBy(Option<PlayerId>, Unit),
}

#[derive(Copy, Clone, Debug)]
pub enum GainedTowerReason {
    CapturedFrom(Option<PlayerId>),
    Explored,
    Spawned,
}

#[derive(Copy, Clone, Debug)]
pub enum LostTowerReason {
    CapturedBy(Option<PlayerId>),
    DestroyedBy(Option<PlayerId>),
    /// The owner was killed.
    PlayerKilled,
}
