// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::alerts::Alerts;
use crate::chunk::ChunkRectangle;
use crate::death_reason::DeathReason;
use crate::force::Path;
use crate::tower::{TowerArray, TowerId, TowerRectangle, TowerType};
use kodiak_common::bitcode::{self, *};
use kodiak_common::{PlayerAlias, PlayerId};

#[derive(Clone, Debug, Encode, Decode)]
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
    Spawn(PlayerAlias),
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
#[derive(Debug, Default, Decode, Encode)]
pub struct NonActor {
    /// Is alive?
    pub alive: bool,
    /// Alerts.
    pub alerts: Alerts,
    /// Clamped to u16::MAX. Doesn't count upgrading towers.
    pub tower_counts: TowerArray<u16>,
    /// Death reason (if dead).
    pub death_reason: Option<DeathReason>,
    /// An approximation of inhabited towers.
    pub bounding_rectangle: TowerRectangle,
}

/// Game server to game client update.
#[derive(Debug, Encode, Decode)]
pub struct Update {
    /// Actor model update.
    pub actor_update: crate::world::ActorUpdate,
    /// Updates the client's [`NonActor`].
    /// contains many small signed/unsigned integers.
    pub non_actor: NonActor,
}
