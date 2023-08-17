// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::hash::CompatHasher;
use core_protocol::prelude::*;
use core_protocol::{PlayerId, TeamId};
use std::collections::BTreeMap;
use std::fmt::Debug;
use std::hash::{Hash, Hasher};

pub use crate::storage::*;
#[doc(hidden)]
pub use paste::paste;

/// Helper macro for common singleton pattern, extracting the `Singleton` from the `World`.
#[macro_export]
macro_rules! singleton {
    ($world:ident) => {
        match $world.singleton.as_ref() {
            None => {
                debug_assert!(
                    $world.is_default(),
                    "missing singleton but has other actors"
                );
                None
            }
            Some(s) => Some(&s.1.actor),
        }
    };
}
pub use singleton;

/// Same as [`singleton`] but mutable.
/// ```ignore
/// // E.g. at the start of World tick:
/// let Some(mut singleton) = singleton_mut!(self) else {
///     return;
/// };
/// ```
#[macro_export]
macro_rules! singleton_mut {
    ($world:ident) => {
        match $world.singleton.as_mut() {
            None => {
                debug_assert!(
                    $world.is_default(),
                    "missing singleton but has other actors"
                );
                None
            }
            Some(s) => Some(&mut s.1.actor),
        }
    };
}
pub use singleton_mut;

/// Shorthand for applying events.
#[macro_export]
macro_rules! apply {
    ($me:ident, $actor:ident, $src:ident, $event:ident, $context:expr) => {
        paste! {
            for state in Map::values_mut(&mut $me.[<$actor:snake>]) {
                for events in state.inbox.[<$src:snake>].values() {
                    state.actor.apply(&events.[<$event:snake>], $context);
                }
            }
        }
    };
}
pub use apply;

/// Shorthand for applying events from [`Server`] aka inputs in [`WorldTick::tick_client`].
#[macro_export]
macro_rules! apply_inputs {
    ($me:ident, $actor:ident, $input:ident, $context:expr) => {
        apply!($me, $actor, Server, $input, $context);
    };
}
pub use apply_inputs;

/// An [`Actor`] identifier.
pub trait ActorId: Copy {
    /// A [`Map`] that:
    /// - supports efficient insertions
    /// - iterates its keys based on [`Ord`]
    /// E.g. a 2d array.
    type DenseMap<T>: Map<Self, T> + Efficient + OrdIter = Self::SparseMap<T>;

    /// A [`Map`] that:
    /// - supports efficient insertions
    /// - iterates its keys based on [`Ord`]
    /// - allocates memory proportional to its len
    /// E.g. a [`HashMap`][`std::collections::HashMap`].
    type SparseMap<T>: Map<Self, T> + Efficient + OrdIter + Sparse;

    /// A [`Map`] that:
    /// - iterates its keys based on [`Ord`]
    /// - allocates memory proportional to its len
    /// E.g. a [`SortedVecMap`].
    type Map<T>: Map<Self, T> + OrdIter + Sparse;
}

impl ActorId for PlayerId {
    type SparseMap<T> = BTreeMap<Self, T>; // TODO better sparse/dense map.
    type Map<T> = SortedVecMap<Self, T>;
}

impl ActorId for TeamId {
    type SparseMap<T> = BTreeMap<Self, T>; // TODO better sparse/dense map.
    type Map<T> = SortedVecMap<Self, T>;
}

// TODO don't require Serialize
#[derive(Copy, Clone, Debug, Default, PartialEq, Serialize, Deserialize, Encode, Decode)]
pub struct Server;

impl ActorId for Server {
    type SparseMap<T> = Option<(Self, T)>; // 1 bit overhead but never used.
    type Map<T> = Wrapper<Self, T>; // 0 bits overhead.
}

/// A discrete unit within the world. The server has all of them and each client has a subset.
pub trait Actor: Clone {
    type Id: ActorId;

    /// How many ticks the [`Actor`] is kept after it is no longer visible.
    const KEEPALIVE: u8 = 5;
}

// Define Apply traits at call site to fix:
// type parameter `T` must be covered by another type when it appears before the first local type (`ActorEventsFromActorId`)
#[doc(hidden)]
#[macro_export]
macro_rules! define_apply {
    () => {
        /// A type that can be mutated by a `&U`. Also takes a context for callbacks.
        pub trait Apply<U, C> {
            fn apply(&mut self, u: &U, context: &mut C);
        }

        /// Like [`Apply`] but takes an owned `U`. TODO find a way to have 1 Apply trait.
        pub trait ApplyOwned<U, C> {
            fn apply_owned(&mut self, u: U, context: &mut C);
        }

        // Allows array based types other than Vec.
        impl<T: Apply<U, C>, U, C, D: std::ops::Deref<Target = [U]>> Apply<D, C> for T {
            fn apply(&mut self, d: &D, context: &mut C) {
                let slice: &[U] = &*d;
                for u in slice {
                    self.apply(u, context);
                }
            }
        }
    };
}

/// An inbox that applies it's [`Message`]s in the same order as they arrive.
pub trait SequentialInbox {}

impl<T, D: std::ops::Deref<Target = [T]>> SequentialInbox for D {}

/// A mutation that can be sent to an [`Actor`].
pub trait Message: Clone {
    type Inbox: Clone + Default + Extend<Self> = Vec<Self>;
}

/// A type that can report if the client has the [`Actor`]s associated with an [`ActorId`].
pub trait IsActive<Id> {
    fn is_active(&self, id: Id) -> bool;

    fn is_inactive(&self, id: Id) -> bool {
        !self.is_active(id)
    }
}

/// Like `Apply` but order does not matter.
pub trait Accumulate<T> {
    fn accumulate(&mut self, t: T);
}

/// A type that provides a level of desync detection.
pub trait Checksum: PartialEq {
    fn diff(&self, server: &Self) -> String;

    /// Can skip accumulates if this returns false.
    fn is_some(&self) -> bool {
        true
    }
}

impl<T> Accumulate<T> for () {
    fn accumulate(&mut self, _: T) {
        // No-op
    }
}

impl Checksum for () {
    fn diff(&self, _: &Self) -> String {
        String::new()
    }

    fn is_some(&self) -> bool {
        false
    }
}

impl<T: Hash> Accumulate<T> for u32 {
    fn accumulate(&mut self, t: T) {
        let mut hasher = CompatHasher::default();
        t.hash(&mut hasher);
        *self ^= hasher.finish() as u32
    }
}

impl Checksum for u32 {
    fn diff(&self, server: &Self) -> String {
        format!("client: {self:?} server: {server:?}")
    }
}

// TODO HashMap/BTreeMap based checksums.

/// Implement on result of [`define_world`] to provide [`tick_client`][`Self::tick_client`].
pub trait WorldTick<C> {
    /// TODO maybe remove everything but tick_client from this trait.
    /// The part of the tick before inputs arrive. Put as much as possible here to reduce latency.
    fn tick_before_inputs(&mut self, context: &mut C);
    /// The part of the tick after inputs are applied. Useful for things which depend on inputs
    /// being applied, such as applying events created by inputs.
    fn tick_after_inputs(&mut self, context: &mut C) {
        let _ = context;
    }
    /// Tick code that gets run on client during update apply.
    fn tick_client(&mut self, context: &mut C);
}

/// A client's knowledge of a particular [`Actor`].
#[derive(Debug)]
pub struct ActorKnowledge {
    /// Starts at [`Self::NEW`], gets set to `keepalive + 1`, then counts down each tick.
    counter: u8,
}

impl Default for ActorKnowledge {
    fn default() -> Self {
        Self { counter: Self::NEW }
    }
}

impl ActorKnowledge {
    /// Sentinel value to indicate that the actor is new.
    const NEW: u8 = u8::MAX;

    /// Was added this tick.
    pub fn is_new(&self) -> bool {
        self.counter == Self::NEW
    }

    /// Can send/receive events. Not [`Self::is_new`] and not [`Self::is_expired`].
    pub fn is_active(&self) -> bool {
        !self.is_new() && !self.is_expired()
    }

    /// Will be removed this tick.
    pub fn is_expired(&self) -> bool {
        self.counter == 0
    }

    /// Called at the beginning up an update. Resets the keepalive. Returns true if it's the first
    /// refresh this tick (not a duplicate).
    pub fn refresh(&mut self, keepalive: u8) -> bool {
        // Start at keepalive + 1 so a keepalive of 0 is valid.
        let counter = keepalive + 1;
        debug_assert_ne!(counter, Self::NEW);

        // Refresh can be called on a new knowledge or multiple times on an existing knowledge if
        // there are duplicates in visibility.
        let is_first = self.counter != counter && !self.is_new();
        if is_first {
            self.counter = counter
        }
        is_first
    }

    /// Called at the beginning of an update.
    pub fn tick(&mut self, keepalive: u8) {
        // Clear sentinel value.
        if self.is_new() {
            // Start at keepalive + 1 so a keepalive of 0 is valid.
            let c = keepalive + 1;
            debug_assert_ne!(c, Self::NEW);
            self.counter = c;
        }

        debug_assert_ne!(self.counter, 0, "expired knowledge wasn't cleared");
        self.counter -= 1;
    }
}

#[macro_export]
macro_rules! define_events {
    ($actor:ident, $src:ident $(, $event:ident)+ $(; $($derive:ident),*)?) => {
        paste! {
            #[derive(Clone, Debug, Default $($(, $derive)*)?)] // TODO clone_from?
            pub(crate) struct [<$actor EventsFrom $src>] {
                $(pub(crate) [<$event:snake>]: <$event as Message>::Inbox),+
            }

            impl<C, T> Apply<[<$actor EventsFrom $src>], C> for T
            where
                T: $(
                    Apply<<$event as Message>::Inbox, C> +
                )+
            {
                fn apply(&mut self, events: &[<$actor EventsFrom $src>], context: &mut C) {
                    $(
                        self.apply(&events.[<$event:snake>], context);
                    )+
                }
            }

            $(
                impl Extend<$event> for [<$actor EventsFrom $src>] {
                    fn extend<I: IntoIterator<Item = $event>>(&mut self, i: I) {
                        self.[<$event:snake>].extend(i);
                    }
                }
            )+
        }
    }
}

// TODO is ActorState the best name? impl Actor is actually the state and this is state + inbox.
#[macro_export]
macro_rules! define_actor_state {
    ($actor:ident $(, $src:ident)* $(; $($derive:ident),*)?) => {
        paste! {
            #[derive(Debug)]
            pub struct [<$actor State>] {
                pub actor: $actor,
                pub(crate) inbox: [<$actor Inbox>],
            }

            impl From<$actor> for [<$actor State>] {
                fn from(actor: $actor) -> Self {
                    Self {
                        actor,
                        inbox: Default::default(),
                    }
                }
            }

            impl<C, I: Message> ApplyOwned<I, C> for [<$actor State>]
            where
                $actor: Apply<I, C>, <I as Message>::Inbox: SequentialInbox,
                [<$actor EventsFromServer>]: Extend<I>,
            {
                fn apply_owned(&mut self, input: I, context: &mut C) {
                    Apply::apply(&mut self.actor, &input, context);
                    self.inbox.server.extend_one(input);
                }
            }

            #[derive(Debug, Default $($(, $derive)*)?)]
            pub struct [<$actor Inbox>] {
                $(pub(crate) [<$src:snake>]: <$src as ActorId>::Map<[<$actor EventsFrom $src>]>),*
            }

            // Optimization: #[derive(Clone)] doesn't implement clone_from.
            impl Clone for [<$actor Inbox>] {
                fn clone(&self) -> Self {
                    Self {
                        $([<$src:snake>]: self.[<$src:snake>].clone()),*
                    }
                }

                fn clone_from(&mut self, source: &Self) {
                    $(self.[<$src:snake>].clone_from(&source.[<$src:snake>]);)*
                }
            }

            impl [<$actor Inbox>] {
                pub fn filter(&self, #[allow(unused)] knowledge: &Knowledge) -> Self {
                    Self {
                        $(
                            [<$src:snake>]: Map::iter(&self.[<$src:snake>]).filter_map(|(id, events)| {
                                knowledge.is_inactive(id).then(|| {
                                    (id, events.clone())
                                })
                            }).collect(),
                        )*
                    }
                }
            }

            $(
                impl<T> Extend<($src, T)> for [<$actor Inbox>]
                    where [<$actor EventsFrom $src>]: Extend<T>
                {
                    fn extend<I: IntoIterator<Item = ($src, T)>>(&mut self, i: I) {
                        for (id, t) in i {
                            self.[<$src:snake>].or_default(id).extend_one(t);
                        }
                    }
                }
            )*
        }
    }
}

#[macro_export]
macro_rules! define_world {
    ($checksum:ty, $($actor:ident),+ $(; $($derive:ident),*)?) => {
        $crate::define_apply!();

        paste! {
            #[derive(Debug, Default)]
            pub struct World {
                $(pub [<$actor:snake>]: <<$actor as Actor>::Id as ActorId>::DenseMap<[<$actor State>]>),+
            }

            impl World {
                /// Clears all the inboxes without modifying the actual state.
                /// TODO(debug_assertions) make sure this gets called between each tick.
                pub fn post_update(&mut self) {
                    $(
                        Map::verify_ord_iter(&self.[<$actor:snake>]);
                        for actor_state in Map::values_mut(&mut self.[<$actor:snake>]) {
                            actor_state.inbox.clone_from(&Default::default());
                        }
                    )+
                }

                /// Gets an update for a client given it's knowledge.
                pub fn get_update<$([<$actor T>]: IntoIterator<Item = <$actor as Actor>::Id>), +>(
                    &self,
                    knowledge: &mut Knowledge,
                    visibility: Visibility<$(impl FnOnce(&Knowledge) -> [<$actor T>]),+>,
                ) -> Update {
                    let mut update = Update::default();

                    $(
                        let mut removals_len = 0;
                        Map::verify_ord_iter(&knowledge.[<$actor:snake>]);
                        for (actor_id, knowledge) in Map::iter_mut(&mut knowledge.[<$actor:snake>]) {
                            knowledge.tick(<$actor as Actor>::KEEPALIVE);
                            let remove = knowledge.is_expired() || !Map::contains(&self.[<$actor:snake>], actor_id);
                            removals_len += remove as usize;
                        }

                        let mut completes_len = 0;
                        for actor_id in (visibility.[<$actor:snake>])(&knowledge) {
                            debug_assert!(Map::contains(&self.[<$actor:snake>], actor_id), "visible actor does not exist");
                            // TODO Map::get_or_insert_with.
                            if let Some(knowledge) = Map::get_mut(&mut knowledge.[<$actor:snake>], actor_id) {
                                let before = knowledge.is_expired();
                                if knowledge.refresh(<$actor as Actor>::KEEPALIVE) {
                                    if before && !knowledge.is_expired() {
                                        removals_len -= 1;
                                    }
                                }
                            } else {
                                Map::insert(&mut knowledge.[<$actor:snake>], actor_id, Default::default());
                                completes_len += 1;
                            }
                        }

                        // Calculate exact size of Box<[T]>s to only allocate once.
                        let actor_knowledge = &mut knowledge.[<$actor:snake>];
                        let inboxes_len = Map::len(actor_knowledge) - completes_len - removals_len;
                        // println!("{:<10}: new {completes_len:>2}, alive {inboxes_len:>2}, expired {removals_len:>2}", stringify!($actor));

                        if removals_len != 0 {
                            let mut removals = Vec::with_capacity(removals_len);
                            Map::retain(actor_knowledge, |actor_id, knowledge| {
                                let remove = knowledge.is_expired() || !Map::contains(&self.[<$actor:snake>], actor_id);
                                if remove {
                                    removals.push_within_capacity(actor_id).unwrap();
                                }
                                !remove
                            });

                            debug_assert_eq!(removals.len(), removals_len);
                            update.[<$actor:snake _removals>] = removals.into_boxed_slice();
                        }

                        // Save variables for next block.
                        let [<$actor:snake _lens>] = (completes_len, inboxes_len);
                    )+

                    $(
                        // Use variables from previous block.
                        let (completes_len, inboxes_len) = [<$actor:snake _lens>];
                        let mut completes = Vec::with_capacity(completes_len);
                        let mut inboxes = Vec::with_capacity(inboxes_len);

                        for (actor_id, k) in Map::iter(&knowledge.[<$actor:snake>]) {
                            let actor_state = Map::get(&self.[<$actor:snake>], actor_id).unwrap_or_else(|| {
                                panic!("knowledge of nonexistent actor: {actor_id:?}");
                            });
                            if Checksum::is_some(&update.checksum) {
                                Accumulate::accumulate(&mut update.checksum, (actor_id, &actor_state.actor));
                            }

                            if k.is_new() {
                                completes.push_within_capacity((actor_id, actor_state.actor.clone())).unwrap();
                            } else {
                                inboxes.push_within_capacity(actor_state.inbox.filter(knowledge)).unwrap();
                            }
                        }

                        debug_assert_eq!(completes.len(), completes_len);
                        debug_assert_eq!(inboxes.len(), inboxes_len);
                        update.[<$actor:snake _completes>] = completes.into_boxed_slice();
                        update.[<$actor:snake _inboxes>] = inboxes.into_boxed_slice();
                    )+
                    update
                }

                /// Checks if `self == Self::default()` without requiring `PartialEq`.
                #[allow(unused)]
                pub fn is_default(&self) -> bool {
                    $(self.[<$actor:snake>].is_empty())&&+
                }
            }

            // Ignores messages sent to actors that aren't visible/don't exist.
            $(
                impl<T> Extend<(<$actor as Actor>::Id, T)> for World
                    where [<$actor Inbox>]: Extend<T>
                {
                    fn extend<I: IntoIterator<Item = (<$actor as Actor>::Id, T)>>(&mut self, i: I) {
                        for (dst, t) in i {
                            if let Some(actor_state) = Map::get_mut(&mut self.[<$actor:snake>], dst) {
                                actor_state.inbox.extend_one(t)
                            }
                        }
                    }
                }
            )*

            #[derive(Debug, Default $($(, $derive)*)?)]
            pub struct Update {
                checksum: $checksum,
                $( // TODO Box may be wasteful for types like Singleton which have at most 1 entry.
                    [<$actor:snake _completes>]: Box<[(<$actor as Actor>::Id, $actor)]>,
                    [<$actor:snake _inboxes>]: Box<[[<$actor Inbox>]]>,
                    [<$actor:snake _removals>]: Box<[<$actor as Actor>::Id]>,
                )+
            }

            impl<C> ApplyOwned<Update, C> for World
            where
                World: WorldTick<C>,
            {
                fn apply_owned(&mut self, update: Update, context: &mut C) {
                    // Do removals and copy inboxes. TODO better error handling.
                    $(
                        for &removal in update.[<$actor:snake _removals>].iter() {
                            Map::remove(&mut self.[<$actor:snake>], removal).expect("removals: actor doesn't exist");
                        }

                        let actors = &mut self.[<$actor:snake>];
                        let actor_inboxes = update.[<$actor:snake _inboxes>];
                        assert_eq!(Map::len(actors), actor_inboxes.len(), "inboxes: length mismatch");

                        for (actor, inbox) in Map::values_mut(actors).zip(Vec::from(actor_inboxes)) {
                            actor.inbox = inbox;
                        }
                    )+

                    WorldTick::tick_client(self, context);

                    // Do completes.
                    $(
                        for (id, complete) in Vec::from(update.[<$actor:snake _completes>]) {
                            let previous = Map::insert(&mut self.[<$actor:snake>], id, complete.into());
                            assert!(previous.is_none(), "complete: actor already exists");
                        }
                    )+

                    let mut checksum = <$checksum>::default();
                    if Checksum::is_some(&checksum) {
                        $(
                            for (actor_id, actor_state) in Map::iter(&self.[<$actor:snake>]) {
                                Accumulate::accumulate(&mut checksum, (actor_id, &actor_state.actor));
                            }
                        )+
                    }

                    if &checksum != &update.checksum {
                        panic!("desync {}", Checksum::diff(&checksum, &update.checksum))
                    }
                }
            }

            /// What part of the world a client knows about.
            #[derive(Debug, Default)]
            pub struct Knowledge {
                $(pub [<$actor:snake>]: <<$actor as Actor>::Id as ActorId>::SparseMap<ActorKnowledge>),*
            }

            $(
                impl IsActive<<$actor as Actor>::Id> for Knowledge {
                    fn is_active(&self, id: <$actor as Actor>::Id) -> bool {
                        Map::get(&self.[<$actor:snake>], id).is_some_and(|k| k.is_active())
                    }
                }
            )+

            /// Events from [`Server`] are always sent.
            impl IsActive<Server> for Knowledge {
                fn is_active(&self, _: Server) -> bool {
                    false
                }
            }

            /// Which actors a client can see this frame. Can contain duplicates.
            pub struct Visibility<$($actor),+> {
                $(pub [<$actor:snake>]: $actor),+
            }
        }
    }
}

// Invariant: contents are sorted.
#[derive(Copy, Clone, Ord, PartialOrd, Eq, PartialEq, Hash, Debug, Encode, Decode)]
pub struct Pair<Id>([Id; 2]);

impl<Id: ActorId + PartialOrd> Pair<Id> {
    pub fn one(a: Id) -> Self {
        Self([a, a])
    }

    pub fn one_or_two(a: Id, b: Id) -> Self {
        if a < b {
            Self([a, b])
        } else {
            Self([b, a])
        }
    }
}

impl<Id: ActorId + Ord> ActorId for Pair<Id> {
    type DenseMap<T> = NonexistentMap<Self, T>;
    type SparseMap<T> = BTreeMap<Self, T>;
    type Map<T> = SortedVecMap<Self, T>;
}

impl<T, Id: ActorId> IsActive<Pair<Id>> for T
where
    T: IsActive<Id>,
{
    fn is_active(&self, id: Pair<Id>) -> bool {
        self.is_active(id.0[0]) && self.is_active(id.0[1])
    }
}

pub use define_actor_state;
pub use define_events;
pub use define_world;

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    #[derive(
        Copy,
        Clone,
        Debug,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        Hash,
        Serialize,
        Deserialize,
        Encode,
        Decode,
    )]
    pub struct SingletonId;

    impl ActorId for SingletonId {
        type SparseMap<T> = Option<(Self, T)>;
        type Map<T> = Option<(Self, T)>;
    }

    #[derive(Clone, Debug, Default, Hash, Serialize, Deserialize, Encode, Decode)]
    pub struct Singleton {
        tick: u32,
        post_tick: u32,
    }

    impl Actor for Singleton {
        type Id = SingletonId;
        const KEEPALIVE: u8 = 0;
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
    enum SingletonInput {}

    impl<C> Apply<SingletonInput, C> for Singleton {
        fn apply(&mut self, _: &SingletonInput, _: &mut C) {}
    }

    impl Message for SingletonInput {}

    #[derive(
        Copy,
        Clone,
        Debug,
        Eq,
        PartialEq,
        Ord,
        PartialOrd,
        Hash,
        Serialize,
        Deserialize,
        Encode,
        Decode,
    )]
    pub struct SectorId(u32);

    impl ActorId for SectorId {
        type SparseMap<T> = BTreeMap<Self, T>;
        type Map<T> = SortedVecMap<Self, T>;
    }

    #[derive(Clone, Debug, Hash, Serialize, Deserialize, Encode, Decode)]
    pub struct Sector {
        data: Vec<u32>,
    }

    impl Actor for Sector {
        type Id = SectorId;
    }

    #[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
    struct SectorInput {
        push: Vec<u32>,
    }

    impl<C> Apply<SectorInput, C> for Sector {
        fn apply(&mut self, input: &SectorInput, _context: &mut C) {
            self.data.extend(&input.push);
        }
    }

    impl Message for SectorInput {}

    #[derive(Clone, Debug, Serialize, Deserialize, Encode, Decode)]
    struct SectorEvent {
        pop: usize,
    }

    impl<C> Apply<SectorEvent, C> for Sector {
        fn apply(&mut self, event: &SectorEvent, _context: &mut C) {
            self.data.drain(..event.pop.min(self.data.len()));
        }
    }

    impl Message for SectorEvent {}

    define_events!(Singleton, Server, SingletonInput; Serialize, Deserialize, Encode, Decode);
    define_actor_state!(Singleton, Server; Serialize, Deserialize, Encode, Decode);
    define_events!(Sector, Server, SectorInput; Serialize, Deserialize, Encode, Decode);
    define_events!(Sector, SectorId, SectorEvent; Serialize, Deserialize, Encode, Decode);
    define_actor_state!(Sector, Server, SectorId; Serialize, Deserialize, Encode, Decode);
    define_world!(u32, Singleton, Sector; Serialize, Deserialize, Encode, Decode);

    const VISIBLE_ID: SectorId = SectorId(1);
    const OTHER_ID: SectorId = SectorId(5);

    impl<C> WorldTick<C> for World {
        fn tick_before_inputs(&mut self, _: &mut C) {
            let Some(singleton) = singleton_mut!(self) else {
                return;
            };
            singleton.tick += 1;

            {
                let has_other = Map::get(&self.sector, OTHER_ID).is_some();

                if let Some(sector_state) = Map::get_mut(&mut self.sector, VISIBLE_ID) {
                    let pop = (singleton.tick / 2) as usize;

                    sector_state
                        .inbox
                        .extend_one((VISIBLE_ID, SectorEvent { pop: 1 }));

                    if has_other {
                        sector_state
                            .inbox
                            .extend_one((OTHER_ID, SectorEvent { pop }));
                    }
                }
            }

            // Don't apply events yet.
        }

        fn tick_after_inputs(&mut self, context: &mut C) {
            let Some(singleton) = singleton_mut!(self) else {
                return;
            };
            singleton.post_tick += 1;

            // Apply events.
            apply!(self, Sector, SectorId, SectorEvent, context);
        }

        fn tick_client(&mut self, context: &mut C) {
            self.tick_before_inputs(context);
            apply_inputs!(self, Singleton, SingletonInput, context);
            apply_inputs!(self, Sector, SectorInput, context);
            self.tick_after_inputs(context);
        }
    }

    #[test]
    fn test() {
        let mut world = World::default();

        Map::insert(
            &mut world.singleton,
            SingletonId,
            Singleton::default().into(),
        );
        Map::insert(
            &mut world.sector,
            VISIBLE_ID,
            Sector {
                data: vec![1, 2, 3],
            }
            .into(),
        );
        Map::insert(
            &mut world.sector,
            OTHER_ID,
            Sector { data: vec![42] }.into(),
        );

        let mut client = Knowledge::default();
        let mut client_world = World::default();

        for i in 0..10u32 {
            let tick = singleton!(world).unwrap().tick;
            println!("\nTICK {tick}");

            world.tick_before_inputs(&mut ());
            if let Some(sector_state) = Map::get_mut(&mut world.sector, VISIBLE_ID) {
                let n = tick * 3;

                sector_state.apply_owned(
                    SectorInput {
                        push: vec![n + 4, n + 5, n + 6],
                    },
                    &mut (),
                );
            }
            world.tick_after_inputs(&mut ());

            println!("server: {world:?}");

            let update = world.get_update(
                &mut client,
                Visibility {
                    singleton: |_: &_| Some(SingletonId),
                    sector: |_: &_| (tick == 0).then_some(VISIBLE_ID),
                },
            );
            world.post_update();

            let update = if i % 2 == 0 {
                bitcode::decode(&bitcode::encode(&update).unwrap()).unwrap()
            } else {
                bitcode::deserialize(&bitcode::serialize(&update).unwrap()).unwrap()
            };

            println!("update: {update:?}");
            client_world.apply_owned(update, &mut ());
            println!("client: {client_world:?}");
        }
    }
}

#[cfg(test)]
mod tests2 {
    use super::*;
    use rand::prelude::IteratorRandom;
    use rand::{thread_rng, Rng};
    use std::collections::BTreeMap;
    use std::fmt::Write;

    #[test]
    fn fuzz() {
        define_events!(Simple, Server, SimpleInput);
        define_events!(Simple, SimpleId, SimpleEvent);
        define_actor_state!(Simple, Server, SimpleId);
        define_world!(u32, Simple);

        impl<C: OnInfo> WorldTick<C> for World {
            fn tick_before_inputs(&mut self, context: &mut C) {
                let mut simple_events = vec![];

                for (simple_id, actor_state) in Map::iter_mut(&mut self.simple) {
                    let actor = &mut actor_state.actor;

                    if actor.0.len() % 3 == 0 {
                        let c = 'm';
                        actor.0.push(c);
                        context.on_info(Info::CharPushed { c, new: &actor });
                    } else {
                        let c = actor.0.pop();
                        context.on_info(Info::CharPopped { c, new: &actor });
                    }

                    if simple_id.0 % 4 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::Overwrite { str: "ABCDE" }));
                    }
                    if simple_id.0 % 8 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::Overwrite { str: "________" }));
                    }
                    if simple_id.0 % 3 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::PushChar { c: 'a' }));
                    } else if actor.0.len() % 7 == 0 {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::Overwrite { str: "abcd" }));
                    } else {
                        actor_state
                            .inbox
                            .extend_one((simple_id, SimpleEvent::PopChar));

                        let dst = SimpleId(simple_id.0.saturating_sub(1));
                        simple_events
                            .extend_one((dst, (simple_id, SimpleEvent::PushChar { c: 'b' })))
                    }
                }

                self.extend(simple_events);
                apply!(self, Simple, SimpleId, SimpleEvent, context);
            }

            fn tick_client(&mut self, context: &mut C) {
                self.tick_before_inputs(context);
                apply_inputs!(self, Simple, SimpleInput, context);
                self.tick_after_inputs(context);
            }
        }

        #[derive(Clone, Debug)]
        enum SimpleInput {
            PushChar { c: char },
        }

        impl Message for SimpleInput {}

        #[derive(Clone, Debug)]
        enum SimpleEvent {
            PushChar { c: char },
            PopChar,
            Overwrite { str: &'static str },
        }

        impl Message for SimpleEvent {}

        impl<C: OnInfo> Apply<SimpleEvent, C> for Simple {
            fn apply(&mut self, event: &SimpleEvent, context: &mut C) {
                match event {
                    &SimpleEvent::PushChar { c, .. } => {
                        self.0.push(c);
                        context.on_info(Info::CharPushed { c, new: &self })
                    }
                    SimpleEvent::PopChar => {
                        let c = self.0.pop();
                        context.on_info(Info::CharPopped { c, new: &self });
                    }
                    SimpleEvent::Overwrite { str } => {
                        self.0.clear();
                        self.0.push_str(str);
                        context.on_info(Info::Overwritten { new: &self });
                    }
                }
            }
        }

        #[derive(Debug)]
        #[allow(unused)]
        enum Info<'a> {
            CharPushed { c: char, new: &'a Simple },
            CharPopped { c: Option<char>, new: &'a Simple },
            Overwritten { new: &'a Simple },
        }

        trait OnInfo {
            fn on_info(&mut self, info: Info);
        }

        #[derive(Copy, Clone, Ord, Hash, Eq, PartialEq, PartialOrd, Debug)]
        pub struct SimpleId(u8);

        impl ActorId for SimpleId {
            type SparseMap<T> = BTreeMap<Self, T>;
            type Map<T> = SortedVecMap<Self, T>;
        }

        #[derive(Clone, Hash, Debug)]
        pub struct Simple(String);

        impl Actor for Simple {
            type Id = SimpleId;
        }

        impl<C: OnInfo> Apply<SimpleInput, C> for Simple {
            fn apply(&mut self, input: &SimpleInput, context: &mut C) {
                match input {
                    &SimpleInput::PushChar { c } => {
                        self.0.push(c);

                        context.on_info(Info::CharPushed { c, new: &self });
                    }
                }
            }
        }

        #[derive(Default)]
        struct Client {
            world: World,
            data: Knowledge,
        }

        fn update_clients(server: &World, clients: &mut [Client], context: &mut impl OnInfo) {
            let n_clients = clients.len();
            for (i, client) in clients.iter_mut().enumerate() {
                let update = server.get_update(
                    &mut client.data,
                    Visibility {
                        simple: |_: &_| {
                            Map::iter(&server.simple).map(|(k, _)| k).filter(move |&n| {
                                thread_rng().gen_bool(if n.0 as usize % n_clients == i {
                                    0.9
                                } else {
                                    0.1
                                })
                            })
                        },
                    },
                );
                client.world.apply_owned(update, context);
            }
        }

        let mut rng = thread_rng();
        let isolate = false;

        #[derive(Default)]
        struct Context;

        impl OnInfo for Context {
            fn on_info(&mut self, i: Info) {
                writeln!(self, "Info: {i:?}").unwrap();
            }
        }

        const DEBUG: bool = false;
        impl Write for Context {
            fn write_str(&mut self, s: &str) -> std::fmt::Result {
                self.write_fmt(format_args!("{s}"))
            }

            fn write_fmt(self: &mut Self, args: std::fmt::Arguments<'_>) -> std::fmt::Result {
                if DEBUG {
                    print!("{args}");
                }
                Ok(())
            }
        }

        let mut context = Context;

        for i in 0..512 {
            writeln!(&mut context, "@@@@@@@@@@@@@@@@@@@@@@@@ FUZZ #{i}").unwrap();

            let mut server = World::default();
            let mut clients = std::iter::repeat_with(Client::default)
                .take(if isolate { 1 } else { rng.gen_range(0..=32) })
                .collect::<Vec<_>>();

            let mut possible_ids = if isolate {
                vec![22, 23]
            } else {
                (0..32).collect::<Vec<_>>()
            };

            for j in 0..rng.gen_range(1..=16) {
                writeln!(&mut context, "@@@@@@@@@@@@@@@ ITERATION #{j}").unwrap();
                writeln!(&mut context, "@@@@@@@ DISPATCH").unwrap();

                for _ in 0..rng.gen_range(0..=4) {
                    if possible_ids.is_empty() {
                        break;
                    }

                    let i = rng.gen_range(0..possible_ids.len());
                    let id = possible_ids.swap_remove(i);
                    server
                        .simple
                        .insert(SimpleId(id), Simple(i.to_string()).into());
                }

                writeln!(&mut context, "@@@@@@@ DISPATCH 2").unwrap();

                if !Map::is_empty(&server.simple) {
                    for _ in 0..rng.gen_range(0..=if isolate { 3 } else { 25 }) {
                        Map::values_mut(&mut server.simple)
                            .choose(&mut rng)
                            .unwrap()
                            .apply_owned(
                                SimpleInput::PushChar {
                                    c: rng.gen_range('0'..='9'),
                                },
                                &mut context,
                            );
                    }
                }
                server.tick_after_inputs(&mut context);

                writeln!(&mut context, "@@@@@@@ UPDATE CLIENTS").unwrap();

                update_clients(&mut server, &mut clients, &mut context);
                server.post_update();

                writeln!(&mut context, "@@@@@@@ TICK: {server:?}").unwrap();

                server.tick_before_inputs(&mut context);
            }
        }
    }
}
