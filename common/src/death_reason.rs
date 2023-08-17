// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::unit::Unit;
use core_protocol::name::PlayerAlias;
use core_protocol::prelude::*;
use diff::Diff;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeathReason {
    RulerKilled {
        /// Is [`None`] if was killed by zombies.
        alias: Option<PlayerAlias>,
        unit: Unit,
    },
}

/// Wraps [`Option<DeathReason>`]. Required to override [`Diff`].
#[derive(Copy, Clone, Debug)]
pub struct OptionDeathReason(pub Option<DeathReason>);

impl From<Option<DeathReason>> for OptionDeathReason {
    fn from(v: Option<DeathReason>) -> Self {
        Self(v)
    }
}

impl From<OptionDeathReason> for Option<DeathReason> {
    fn from(v: OptionDeathReason) -> Self {
        v.0
    }
}

// Send [`DeathReason`]s with minimal diffing.
#[derive(Debug, Serialize, Deserialize)]
#[doc(hidden)]
pub enum OptionDeathReasonDiff {
    Some(DeathReason),
    None,
    NoChange,
}

impl Diff for OptionDeathReason {
    type Repr = OptionDeathReasonDiff;

    fn diff(&self, other: &Self) -> Self::Repr {
        match (&self.0, &other.0) {
            (None, None) => Self::Repr::NoChange,
            (None, Some(t)) => Self::Repr::Some(*t),
            (Some(_), None) => Self::Repr::None,
            (Some(old), Some(new)) => {
                if old == new {
                    Self::Repr::NoChange
                } else {
                    Self::Repr::Some(*new)
                }
            }
        }
    }

    fn apply(&mut self, diff: &Self::Repr) {
        match diff {
            Self::Repr::Some(new) => self.0 = Some(*new),
            Self::Repr::None => self.0 = None,
            Self::Repr::NoChange => (),
        }
    }

    fn identity() -> Self {
        Self(None)
    }
}
