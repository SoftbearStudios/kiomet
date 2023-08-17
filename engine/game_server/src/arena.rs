use crate::{context_service::ContextService, game_service::GameArenaService};
use core_protocol::RealmName;
use std::collections::HashMap;

pub(crate) struct ArenaRepo<G: GameArenaService> {
    main: ContextService<G>,
    realms: HashMap<RealmName, ContextService<G>>,
}

#[allow(unused)]
impl<G: GameArenaService> ArenaRepo<G> {
    pub(crate) fn new(main: ContextService<G>) -> Self {
        Self {
            main,
            realms: HashMap::new(),
        }
    }

    pub(crate) fn main(&self) -> &ContextService<G> {
        &self.main
    }

    pub(crate) fn main_mut(&mut self) -> &mut ContextService<G> {
        &mut self.main
    }

    pub(crate) fn get(&self, realm_name: Option<RealmName>) -> Option<&ContextService<G>> {
        if let Some(realm_name) = realm_name {
            self.realms.get(&realm_name)
        } else {
            Some(&self.main)
        }
    }

    pub(crate) fn get_mut(
        &mut self,
        realm_name: Option<RealmName>,
    ) -> Option<&mut ContextService<G>> {
        if let Some(realm_name) = realm_name {
            self.realms.get_mut(&realm_name)
        } else {
            Some(&mut self.main)
        }
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (Option<RealmName>, &ContextService<G>)> {
        std::iter::once((None, &self.main))
            .chain(self.realms.iter().map(|(id, cs)| (Some(*id), cs)))
    }

    pub(crate) fn iter_mut(
        &mut self,
    ) -> impl Iterator<Item = (Option<RealmName>, &mut ContextService<G>)> {
        std::iter::once((None, &mut self.main))
            .chain(self.realms.iter_mut().map(|(id, cs)| (Some(*id), cs)))
    }
}
