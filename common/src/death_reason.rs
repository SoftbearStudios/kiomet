// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::unit::Unit;
use kodiak_common::bitcode::{self, *};
use kodiak_common::PlayerAlias;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Encode, Decode)]
pub enum DeathReason {
    RulerKilled {
        /// Is [`None`] if was killed by zombies.
        alias: Option<PlayerAlias>,
        unit: Unit,
    },
}
