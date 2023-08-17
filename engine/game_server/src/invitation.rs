// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::game_service::GameArenaService;
use crate::player::{PlayerData, PlayerRepo};
use crate::unwrap_or_return;
use atomic_refcell::AtomicRefMut;
use core_protocol::dto::InvitationDto;
use core_protocol::id::{InvitationId, PlayerId, ServerId};
use core_protocol::rpc::{InvitationRequest, InvitationUpdate};
use core_protocol::RealmName;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::marker::PhantomData;

/// Invitations, shared by all arenas.
pub struct InvitationRepo<G: GameArenaService> {
    // TODO: Prune.
    invitations: HashMap<InvitationId, Invitation>,
    _spooky: PhantomData<G>,
}

/// For routing invitations.
#[derive(Clone, Debug)]
pub struct Invitation {
    /// Sender arena id.
    pub realm_name: Option<RealmName>,
    /// Sender.
    pub player_id: PlayerId,
}

/// Invitation related data stored in player.
#[derive(Debug)]
pub struct ClientInvitationData {
    /// Incoming invitation accepted by player.
    pub invitation_accepted: Option<InvitationDto>,
    /// Outgoing invitation created by player.
    pub invitation_created: Option<InvitationId>,
}

impl ClientInvitationData {
    pub fn new(invitation_accepted: Option<InvitationDto>) -> Self {
        Self {
            invitation_accepted,
            invitation_created: None,
        }
    }
}

impl<G: GameArenaService> Default for InvitationRepo<G> {
    fn default() -> Self {
        Self {
            invitations: HashMap::new(),
            _spooky: PhantomData,
        }
    }
}

impl<G: GameArenaService> InvitationRepo<G> {
    /// Looks up an invitation by id.
    pub fn get(&self, invitation_id: InvitationId) -> Option<&Invitation> {
        self.invitations.get(&invitation_id)
    }

    /// Returns how many invitations are cached.
    #[allow(clippy::len_without_is_empty)]
    pub fn len(&self) -> usize {
        self.invitations.len()
    }

    /// Forgets any invitation the player created.
    pub(crate) fn forget_player_invitation(&mut self, player: &mut AtomicRefMut<PlayerData<G>>) {
        let client = unwrap_or_return!(player.client_mut());
        if let Some(invitation_id) = client.invitation.invitation_created {
            let removed = self.invitations.remove(&invitation_id);
            debug_assert!(removed.is_some(), "invitation was cleared elsewhere");
            client.invitation.invitation_created = None;
        }
    }

    fn accept(
        &self,
        req_player_id: PlayerId,
        invitation_id: InvitationId,
        players: &mut PlayerRepo<G>,
    ) -> Result<InvitationUpdate, &'static str> {
        let mut req_player = players
            .borrow_player_mut(req_player_id)
            .ok_or("req player doesn't exist")?;

        let req_client = req_player
            .client_mut()
            .ok_or("only clients can accept invitations")?;

        req_client.invitation.invitation_accepted =
            self.invitations
                .get(&invitation_id)
                .map(|invitation| InvitationDto {
                    player_id: invitation.player_id,
                });
        if req_client.invitation.invitation_accepted.is_some() {
            Ok(InvitationUpdate::Accepted)
        } else {
            Err("no such invitation")
        }
    }

    /// Requests an invitation id (new or recycled).
    fn create(
        &mut self,
        req_player_id: PlayerId,
        realm_name: Option<RealmName>,
        server_id: ServerId,
        players: &mut PlayerRepo<G>,
    ) -> Result<InvitationUpdate, &'static str> {
        let mut req_player = players
            .borrow_player_mut(req_player_id)
            .ok_or("req player doesn't exist")?;

        let req_client = req_player
            .client_mut()
            .ok_or("only clients can request invitations")?;

        // Silently ignore case of previously created invitation id.
        let invitation_id = if let Some(invitation_id) = req_client.invitation.invitation_created {
            invitation_id
        } else {
            loop {
                let invitation_id = InvitationId::generate(server_id.cloud_server_number());
                if let Entry::Vacant(entry) = self.invitations.entry(invitation_id) {
                    entry.insert(Invitation {
                        realm_name,
                        player_id: req_player_id,
                    });
                    req_client.invitation.invitation_created = Some(invitation_id);
                    break invitation_id;
                }
            }
        };

        Ok(InvitationUpdate::Created(invitation_id))
    }

    pub fn handle_invitation_request(
        &mut self,
        player_id: PlayerId,
        request: InvitationRequest,
        realm_name: Option<RealmName>,
        server_id: ServerId,
        players: &mut PlayerRepo<G>,
    ) -> Result<InvitationUpdate, &'static str> {
        match request {
            InvitationRequest::Accept(invitation_id) => {
                self.accept(player_id, invitation_id, players)
            }
            InvitationRequest::Create => self.create(player_id, realm_name, server_id, players),
        }
    }
}
