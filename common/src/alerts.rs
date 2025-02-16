// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::tower::TowerId;
use flagset::{flags, FlagSet};
use kodiak_common::bitcode::{self, *};

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Encode, Decode)]
pub struct Alerts {
    /// Approximate ruler position, if known.
    pub ruler_position: Option<TowerId>,
    /// The most damaging tower forces that are currently overflowing.
    pub overflowing: Option<TowerId>,
    /// The most damaging tower forces that are currently full.
    pub full: Option<TowerId>,
    /// Zombies are attacking this tower.
    pub zombies: Option<TowerId>,
    /// Packed bit flags. TODO don't gamma.
    flags: u8,
}

impl Alerts {
    /// Clear the things that should be overwritten upon recalculation.
    pub fn reset_ephemeral(&mut self) {
        self.ruler_position = None;
        self.full = None;
        self.overflowing = None;
        self.zombies = None;
        self.set_flags(self.flags() - (AlertFlag::RulerUnderAttack | AlertFlag::RulerNotSafe));
    }

    pub fn flags(&self) -> FlagSet<AlertFlag> {
        FlagSet::new_truncated(self.flags)
    }

    pub fn set_flags(&mut self, flags: FlagSet<AlertFlag>) {
        self.flags = flags.bits();
    }
}

flags! {
    pub enum AlertFlag: u8 {
        RulerNotSafe,
        RulerUnderAttack,
        DeployedAnyForce,
        UpgradedAnyTower,
        SetAnySupplyLine,
        UnsetAnySupplyLine,
    }
}
