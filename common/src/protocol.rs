// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::alerts::Alerts;
use crate::chunk::ChunkRectangle;
use crate::death_reason::OptionDeathReason;
use crate::force::Path;
use crate::tower::{TowerArray, TowerId, TowerRectangle, TowerType};
use core_protocol::prelude::*;
use core_protocol::PlayerId;
use serde::{Deserialize, Serialize};

pub use diff::Diff;

#[derive(Clone, Encode, Decode)]
pub enum Command {
    Alliance {
        with: PlayerId,
        break_alliance: bool,
    },
    DeployForce {
        tower_id: TowerId,
        path: Path,
    },
    SetSupplyLine {
        tower_id: TowerId,
        path: Option<Path>,
    },
    SetViewport(ChunkRectangle),
    Spawn,
    Upgrade {
        tower_id: TowerId,
        tower_type: TowerType,
    },
}

impl Command {
    pub fn deploy_force_from_path(path: Vec<TowerId>) -> Self {
        Self::DeployForce {
            tower_id: path[0],
            path: Path::new(path),
        }
    }
}

/// Non actor model data that the client needs. Diffed for efficiency.
#[derive(Debug, Diff)]
#[diff(attr(#[derive(Debug, Serialize, Deserialize)]))]
pub struct NonActor {
    /// Is alive?
    pub alive: bool,
    /// Alerts.
    pub alerts: Alerts,
    /// Clamped to 255. Doesn't count upgrading towers.
    pub tower_counts: TowerArray<u8>,
    /// Death reason (if dead).
    pub death_reason: OptionDeathReason,
    /// An approximation of inhabited towers.
    pub bounding_rectangle: TowerRectangle,
}

impl Default for NonActor {
    fn default() -> Self {
        Diff::identity()
    }
}

/// Game server to game client update.
#[derive(Debug, Encode, Decode)]
pub struct Update {
    /// Actor model update.
    pub actor_update: crate::world::Update,
    /// Updates the client's [`NonActor`]. `#[bitcode_hint(gamma)]` works very well since the diff
    /// contains many small signed/unsigned integers.
    /// (TODO)
    #[bitcode(with_serde)]
    pub non_actor_diff: NonActorDiff,
}
