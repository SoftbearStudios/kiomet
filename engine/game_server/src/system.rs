// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::game_service::GameArenaService;
use crate::infrastructure::Infrastructure;
use crate::util::diff_small_n;
use actix::{Handler, Message};
use core_protocol::dto::ServerDto;
use core_protocol::id::{InvitationId, RegionId};
use core_protocol::rpc::{SystemResponse, SystemUpdate};
use core_protocol::ServerNumber;
use rand::{thread_rng, Rng};
use std::marker::PhantomData;
use std::sync::Arc;

/// Monitors web servers and changes DNS to recover from servers going offline.
///
/// System, in this case, refers to a distributed system of multiple servers.
pub struct SystemRepo<G: GameArenaService> {
    /// All servers on the domain, from plasma.
    pub(crate) servers: Box<[ServerDto]>,
    /// Compatible, active servers. For diffing.
    previous: Arc<[ServerDto]>,
    _spooky: PhantomData<G>,
}

impl<G: GameArenaService> SystemRepo<G> {
    pub fn new() -> Self {
        Self {
            servers: Vec::new().into(),
            previous: Vec::new().into(),
            _spooky: PhantomData,
        }
    }

    pub(crate) fn initializer(&self) -> Option<SystemUpdate> {
        (!self.previous.is_empty()).then(|| SystemUpdate::Added(Arc::clone(&self.previous)))
    }

    #[allow(clippy::type_complexity)]
    pub(crate) fn delta(&mut self) -> Option<(Arc<[ServerDto]>, Arc<[ServerNumber]>)> {
        if let Some((added, removed)) =
            diff_small_n(&self.previous, &self.servers, |dto| dto.server_number)
        {
            self.previous = self.servers.iter().cloned().collect();
            Some((added.into(), removed.into()))
        } else {
            None
        }
    }

    /// Iterates available servers, their absolute priorities (lower is higher priority),
    /// and player counts, in an undefined order.
    fn iter_server_priorities(
        system: &SystemRepo<G>,
        requested_server_number: Option<ServerNumber>,
        invitation_server_number: Option<ServerNumber>,
        ideal_region_id: Option<RegionId>,
    ) -> impl Iterator<Item = (ServerNumber, i8, u32)> + '_ {
        system.previous.iter().map(move |server| {
            let mut priority = 0;

            if let Some(ideal_region_id) = ideal_region_id {
                priority = ideal_region_id.distance(server.region_id) as i8;
            }

            if Some(server.server_number) == requested_server_number {
                priority = -1;
            }

            if Some(server.server_number) == invitation_server_number {
                priority = -2;
            }

            (server.server_number, priority, server.player_count)
        })
    }
}

/// Asks the server about the distributed system of servers.
#[derive(Message)]
#[rtype(result = "SystemResponse")]
pub struct SystemRequest {
    /// [`ServerNumber`] preference. `None` means localhost/no preference.
    pub(crate) server_number: Option<ServerNumber>,
    /// [`RegionId`] preference.
    pub(crate) region_id: Option<RegionId>,
    /// [`InvitationId`] server preference.
    pub(crate) invitation_id: Option<InvitationId>,
}

/// Reports whether infrastructure is healthy (hardware and actor are running properly).
impl<G: GameArenaService> Handler<SystemRequest> for Infrastructure<G> {
    type Result = SystemResponse;

    fn handle(&mut self, request: SystemRequest, _: &mut Self::Context) -> Self::Result {
        let invitation_server_number = request.invitation_id.and_then(|id| id.server_number());
        let ideal_region_id = request.region_id;
        let distribute_load = true;
        let unlisted = self.plasma.role.is_unlisted();

        let ideal_server_number = SystemRepo::iter_server_priorities(
            &self.system,
            request.server_number,
            invitation_server_number,
            ideal_region_id,
        )
        .min_by_key(|&(_, priority, player_count)| {
            (priority, if distribute_load { player_count } else { 0 })
        })
        .map(
            |(ideal_server_number, ideal_server_priority, ideal_server_player_count)| {
                if distribute_load {
                    let mut rng = thread_rng();

                    // Prime the RNG a bit.
                    let use_player_count = rng.gen::<bool>();
                    rng.gen::<u64>();

                    use rand::prelude::IteratorRandom;
                    let result = SystemRepo::iter_server_priorities(
                        &self.system,
                        request.server_number,
                        invitation_server_number,
                        ideal_region_id,
                    )
                    .filter(|&(_, priority, player_count)| {
                        priority == ideal_server_priority
                            && (!use_player_count || player_count == ideal_server_player_count)
                    })
                    .map(|(server_id, _, _)| server_id)
                    .choose(&mut rng);

                    if let Some(result) = result {
                        result
                    } else {
                        debug_assert!(false, "server id rug pull");
                        ideal_server_number
                    }
                } else {
                    ideal_server_number
                }
            },
        )
        .filter(|_| !unlisted);

        SystemResponse {
            server_number: ideal_server_number,
        }
    }
}
