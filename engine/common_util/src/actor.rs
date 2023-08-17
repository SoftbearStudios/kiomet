// SPDX-FileCopyrightText: 2022 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::hash::CompatHasher;
use core_protocol::prelude::*;
use std::cmp::Ordering;
use std::collections::hash_map::Entry;
use std::collections::{BTreeMap, HashMap, HashSet};
use std::fmt::{self, Debug, Formatter};
use std::hash::{Hash, Hasher};
use std::marker::PhantomData;
use std::ops::{Deref, DerefMut};

/// A [`Client`] or [`Server`] role consisting of state and one or more passes of behavior.
pub struct World<S, P, R> {
    /// On the server, this is the full state of the world (all partitions).
    /// On the client, this may be a partial state of the world (a subset of the partitions).
    state: S,
    /// The last pass of behavior, which recursively contains previous passes.
    pass: P,
    /// [`Client`] or [`Server`]; may contain role-specific state.
    role: R,
}

impl<S: State + Default, P: Pass + Default, R: Default> Default for World<S, P, R> {
    fn default() -> Self {
        Self::new(S::default())
    }
}

impl<S: State, P: Pass + Default, R: Default> World<S, P, R> {
    pub fn new(state: S) -> Self {
        Self {
            state,
            pass: P::default(),
            role: R::default(),
        }
    }
}

impl<S: State, P: Pass> World<S, P, Server<S>> {
    /// Returns total capacity allocation for events, deletions, etc.
    pub fn capacity(&self) -> usize {
        self.pass.capacity() + self.role.pending.capacity() + self.role.removed.capacity()
    }
}

impl<S: State, P: Pass, R> Deref for World<S, P, R> {
    type Target = S;

    fn deref(&self) -> &S {
        &self.state
    }
}

/// Per-tick server to client update.
#[derive(Serialize, Deserialize, Encode, Decode)]
pub struct Update<S: State, P: Pass> {
    /// Partitions no longer visible to client.
    #[serde(bound(
        serialize = "S::PartitionId: Serialize",
        deserialize = "S::PartitionId: Deserialize<'de>"
    ))]
    #[bitcode(bound_type = "S::PartitionId")]
    deletes: Vec<S::PartitionId>,
    /// Update for visible partitions.
    update: P::Update,
    /// Dispatched events.
    #[serde(bound(
        serialize = "S::Event: Serialize",
        deserialize = "S::Event: Deserialize<'de>"
    ))]
    #[bitcode(bound_type = "S::Event")]
    events: Vec<S::Event>,
    /// Newly visible partitions.
    #[serde(bound(
        serialize = "(S::PartitionId, S::Partition): Serialize",
        deserialize = "(S::PartitionId, S::Partition): Deserialize<'de>"
    ))]
    #[bitcode(bound_type = "(S::PartitionId, S::Partition)")]
    completes: Vec<(S::PartitionId, S::Partition)>,
    /// Checksum after all the above is applied.
    checksum: S::Checksum,
}

impl<S: State, P: Pass> Debug for Update<S, P>
where
    S::PartitionId: Debug,
    S::Event: Debug,
    P::Update: Debug,
    S::Checksum: Debug,
{
    fn fmt(&self, f: &mut Formatter) -> fmt::Result {
        write!(f, "Update{{checksum: {:?} @ ", self.checksum,)?;
        for (i, event) in self
            .deletes
            .iter()
            .map(|partition_id| format!("{partition_id:?}: D"))
            .chain(Some(format!("{:?}", self.update)))
            .chain((!self.events.is_empty()).then(|| format!("{:?}", self.events)))
            .chain(
                self.completes
                    .iter()
                    .map(|(partition_id, _)| format!("{partition_id:?}: A")),
            )
            .enumerate()
        {
            if i > 0 {
                f.write_str(", ")?;
            }
            f.write_str(&event)?;
        }
        write!(f, "]}}")
    }
}

/// Data stored on the server, per client.
pub struct ClientData<S: State> {
    /// Current knowledge of partitions.
    known: HashMap<S::PartitionId, PartitionKnowledge>,
}

impl<S: State> Default for ClientData<S> {
    fn default() -> Self {
        Self {
            known: Default::default(),
        }
    }
}

impl<S: State> ClientData<S> {
    fn is_active(&self, partition_id: S::PartitionId) -> bool {
        self.known
            .get(&partition_id)
            .map_or(false, |k| k.is_active())
    }
}

impl<S: State> Debug for ClientData<S>
where
    S::PartitionId: Debug,
{
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        f.debug_struct("ClientData")
            .field("known", &self.known.keys().collect::<Vec<_>>())
            .finish()
    }
}

/// A client's knowledge of a particular partition.
struct PartitionKnowledge {
    /// Starts at [`Self::NEW`], gets set to `keepalive + 1`, then counts down each tick.
    counter: u8,
}

impl Default for PartitionKnowledge {
    fn default() -> Self {
        Self { counter: Self::NEW }
    }
}

impl PartitionKnowledge {
    /// Sentinel value to indicate that the partition is new.
    const NEW: u8 = u8::MAX;

    /// Was added this tick.
    fn is_new(&self) -> bool {
        self.counter == Self::NEW
    }

    /// Can send/receive events. Not [`Self::is_new`] and not [`Self::is_expired`].
    fn is_active(&self) -> bool {
        !self.is_new() && !self.is_expired()
    }

    /// Will be removed this tick.
    fn is_expired(&self) -> bool {
        self.counter == 0
    }

    /// Called at the beginning up an update. Resets the keepalive.
    fn refresh(&mut self, keepalive: u8) {
        let counter = keepalive + 1;
        debug_assert_ne!(counter, Self::NEW);
        self.counter = counter
    }

    /// Called at the end of an update. Returns true if the partition should be kept.
    fn tick(&mut self, keepalive: u8) -> bool {
        if self.is_new() {
            // Clear sentinel value. Start at keepalive + 1 so a keepalive of 0 is valid.
            let c = keepalive + 1;
            debug_assert_ne!(c, Self::NEW);
            self.counter = c;
        }

        if let Some(c) = self.counter.checked_sub(1) {
            self.counter = c;
            true
        } else {
            false
        }
    }
}

pub trait PassDef {
    /// The state the events are applied to.
    type State: State;
    /// The event produced and consumed by this pass.
    type Event: Clone;
    /// The event returned from this pass to the next pass.
    type OutputEvent;
    /// Iterator returned by `source_partition_ids`.
    // TODO: Use RPITIT once it's ready.
    type SourcePartitionIds: Iterator<Item = <Self::State as State>::PartitionId> =
        std::iter::Once<<Self::State as State>::PartitionId>;

    fn cmp(a: &Self::Event, b: &Self::Event) -> Ordering;

    /// Return which partition an event originated from. Clients with all these partitions are able
    /// to predict such an event.
    fn source_partition_ids(event: &Self::Event) -> Self::SourcePartitionIds;

    /// Return which partition an event affects.
    fn destination_partition_id(event: &Self::Event) -> <Self::State as State>::PartitionId;

    /// Some behavior that may produce events and/or info.
    fn tick(
        state: &mut Self::State,
        on_event: impl FnMut(Self::Event),
        on_info: impl FnMut(<Self::State as State>::Info<'_>),
    );

    /// Sorts the events to ensure determinism.
    fn sort(events: &mut [Self::Event]) {
        events.sort_by(|a, b| {
            Self::cmp(a, b)
                .then_with(|| Self::source_partition_ids(a).cmp(Self::source_partition_ids(b)))
        });
    }

    /// Applies the events from the iterator (must be sorted).
    fn apply(
        state: &mut Self::State,
        events: impl IntoIterator<Item = Self::Event>,
        on_event: impl FnMut(Self::OutputEvent),
        on_info: impl FnMut(<Self::State as State>::Info<'_>),
    );
}

/// Behavior + event storage if needed.
pub trait Pass {
    /// The corresponding state.
    type State: State;
    type Event;
    type OutputEvent;
    /// Update for pass this and, recursively, all previous passes.
    type Update;

    /// If server_update is `Some`, regarded as a client tick. Otherwise regarded a server tick.
    /// on_hash is called during the most appropriate time to hash the state from a client's perspective.
    fn tick(
        &mut self,
        state: &mut Self::State,
        update: Option<Self::Update>,
        on_event: impl FnMut(Self::OutputEvent),
        on_info: impl FnMut(<Self::State as State>::Info<'_>),
    );

    /// Called on the server to get update to pass to clients.
    fn update(&self, client_data: &ClientData<Self::State>) -> Self::Update;

    fn capacity(&self) -> usize;
}

/// Implements [`Pass`] for [`PassDef`].
pub struct PassContext<
    S: State,
    PD: PassDef<State = S>,
    P: Pass<State = S> = PhantomData<(S, <PD as PassDef>::Event)>,
> {
    /// The previous pass and, recursively, all previous passes.
    prev: P,
    /// On the server, this contains events that were immediately applied but possibly need echoing.
    /// On the client, this is a scratch allocation, used during ticks.
    pending: Vec<PD::Event>,
}

impl<S: State, PD: PassDef<State = S>, P: Pass<State = S> + Default> Default
    for PassContext<S, PD, P>
{
    fn default() -> Self {
        Self {
            prev: P::default(),
            pending: Vec::new(),
        }
    }
}

impl<S: State, PD: PassDef<State = S>, P: Pass<State = S>> Deref for PassContext<S, PD, P> {
    type Target = P;

    fn deref(&self) -> &Self::Target {
        &self.prev
    }
}

impl<S: State, PD: PassDef<State = S>, P: Pass<State = S>> DerefMut for PassContext<S, PD, P> {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.prev
    }
}

impl<S: State, PD: PassDef<State = S>, P: Pass<State = S, OutputEvent = PD::Event>> Pass
    for PassContext<S, PD, P>
{
    type State = S;
    type Event = PD::Event;
    type OutputEvent = PD::OutputEvent;
    type Update = (P::Update, Vec<PD::Event>);

    fn tick(
        &mut self,
        state: &mut Self::State,
        update: Option<Self::Update>,
        on_output_event: impl FnMut(Self::OutputEvent),
        mut on_info: impl FnMut(S::Info<'_>),
    ) {
        let pending = &mut self.pending;
        pending.clear();

        let server_update = update.map(|(prev_update, events)| {
            pending.extend(events); // TODO write a test that validates this.
            prev_update
        });

        self.prev.tick(
            state,
            server_update,
            |event| pending.push(event),
            &mut on_info,
        );

        PD::tick(state, |event| pending.push(event), &mut on_info);
        PD::sort(pending);
        PD::apply(state, pending.iter().cloned(), on_output_event, on_info);
    }

    fn update(&self, client_data: &ClientData<Self::State>) -> Self::Update {
        let prev = self.prev.update(client_data);
        let local = self
            .pending // TODO remove this n^2 nonsense.
            .iter()
            .filter(|event| {
                client_data.is_active(PD::destination_partition_id(event))
                    && !PD::source_partition_ids(event)
                        .all(|partition_id| client_data.is_active(partition_id))
            })
            .cloned()
            .collect::<Vec<_>>();
        (prev, local)
    }

    fn capacity(&self) -> usize {
        self.pending.capacity() + self.prev.capacity()
    }
}

// The no-op [`Pass`] at the end of the recursive passes.
impl<S: State, OE> Pass for PhantomData<(S, OE)> {
    type State = S;
    type Event = ();
    type OutputEvent = OE;
    type Update = ();

    fn tick(
        &mut self,
        _: &mut Self::State,
        _: Option<Self::Update>,
        _: impl FnMut(Self::OutputEvent),
        _: impl FnMut(S::Info<'_>),
    ) {
    }

    fn update(&self, _: &ClientData<Self::State>) {}

    fn capacity(&self) -> usize {
        0
    }
}

/// State in need of network synchronization.
pub trait State {
    /// Identifies disjoint subsets of the state.
    type PartitionId: Copy + Ord + Hash;
    /// A disjoint subset of the state.
    type Partition;
    /// An informational event produced but never sent over the network.
    type Info<'a>;
    /// A state-affecting event produced and, if needed, sent over the network.
    type Event: Clone;
    /// A checksum to verify synchronization. May be:
    ///  - [()] (no checksum)
    ///  - [u32] (hash checksum)
    ///  - [BTreeMap<Self::PartitionId, Self::Partition>] (complete checksum)
    ///  - Custom [Checksum] implementation.
    type Checksum: Checksum<Self> = ();

    /// How many ticks to send updates after a partition is no longer visible.
    /// A keepalive of 0 would remove partitions the same tick they leave visibility.
    const PARTITION_KEEPALIVE: u8 = 5;
    /// How many completes may be sent per tick.
    const COMPLETE_QUOTA: usize = usize::MAX;

    /// Return the affected partition.
    fn destination_partition_id(event: &Self::Event) -> Self::PartitionId;

    /// Visit all [`PartitionId`][`Self::PartitionId`]s present in the state (not all theoretical `PartitionId`s)
    // TODO replace with Iterator using RPITIT once it's ready.
    fn visit_partition_ids(&self, visitor: impl FnMut(Self::PartitionId));

    /// Lookup the contents of a partition.
    fn get_partition(&self, partition_id: Self::PartitionId) -> Option<Self::Partition>;

    /// Hash the contents of a partition. Do this more efficiently if possible.
    fn hash_partition<H: Hasher>(&self, partition_id: Self::PartitionId, state: &mut H) {
        let _ = (partition_id, state);
        unreachable!();
    }

    /// Set the contents of a partition, returning the old value.
    fn insert_partition(
        &mut self,
        partition_id: Self::PartitionId,
        partition: Self::Partition,
    ) -> Option<Self::Partition>;

    fn remove_partition(&mut self, partition_id: Self::PartitionId) -> Option<Self::Partition>;

    /// Applies an event to the state.
    fn apply(&mut self, event: Self::Event, on_info: impl FnMut(Self::Info<'_>));

    /// Info that gets printed on desync.
    fn desync_log(&self) -> String {
        String::new()
    }
}

impl<S: State, P: Pass<State = S>> World<S, P, Server<S>> {
    pub fn dispatch(&mut self, event: S::Event, on_info: impl FnMut(S::Info<'_>)) {
        self.role.pending.push(event.clone());
        self.state.apply(event, on_info);
    }

    /// Adds a partition at `partition_id`.
    ///
    /// # Panics
    ///
    /// If the partition already exists.
    pub fn insert_partition(&mut self, partition_id: S::PartitionId, partition: S::Partition) {
        let old = self.state.insert_partition(partition_id, partition);
        assert!(old.is_none());
    }

    /// Removes the partition at `partition_id`.
    ///
    /// # Panics
    ///
    /// If the partition doesn't exist.
    pub fn remove_partition(&mut self, partition_id: S::PartitionId) {
        self.state.remove_partition(partition_id).unwrap();
        self.role.removed.insert(partition_id);
    }

    /// `visibility` can tolerate duplicates.
    pub fn update(
        &self,
        client_data: &mut ClientData<S>,
        visibility: impl IntoIterator<Item = S::PartitionId>,
    ) -> Update<S, P> {
        let mut deletes = Vec::<S::PartitionId>::new();
        let mut completes = Vec::<(S::PartitionId, S::Partition)>::new();

        // TODO replace with HashSet::intersection or similar to fix this n^2.
        for partition_id in &self.role.removed {
            if client_data.known.remove(partition_id).is_some() {
                deletes.push(*partition_id);
            }
        }

        for partition_id in visibility {
            if self.role.removed.contains(&partition_id) {
                continue;
            }

            match client_data.known.entry(partition_id) {
                Entry::Occupied(mut occupied) => {
                    occupied.get_mut().refresh(S::PARTITION_KEEPALIVE);
                }
                Entry::Vacant(vacant) => {
                    if completes.len() >= S::COMPLETE_QUOTA {
                        continue;
                    }

                    vacant.insert(Default::default());
                    completes.push((
                        partition_id,
                        self.state
                            .get_partition(partition_id)
                            .expect("missing visible partition"),
                    ));
                }
            }
        }

        let update = self.pass.update(client_data);

        // TODO remove this n^2 nonsense.
        let events = self
            .role
            .pending
            .iter()
            .filter(|event| client_data.is_active(S::destination_partition_id(event)))
            .cloned()
            .collect::<Vec<_>>();

        let mut checksum = S::Checksum::default();
        client_data.known.retain(|&partition_id, keepalive| {
            if keepalive.tick(S::PARTITION_KEEPALIVE) {
                checksum.accumulate(partition_id, &self.state);
                true
            } else {
                deletes.push(partition_id);
                false
            }
        });

        Update {
            deletes,
            update,
            events,
            completes,
            checksum,
        }
    }

    pub fn tick(&mut self, on_info: impl FnMut(S::Info<'_>)) {
        self.role.pending.clear();
        self.role.removed.clear();
        self.pass.tick(
            &mut self.state,
            None,
            |_| panic!("top-level event"),
            on_info,
        );
    }
}

impl<S: State, P: Pass<State = S>> World<S, P, Client> {
    pub fn tick(&mut self, update: Update<S, P>, mut on_info: impl FnMut(S::Info<'_>)) {
        let Update {
            deletes,
            update,
            events,
            completes,
            checksum: expected_checksum,
        } = update;

        for partition_id in deletes {
            self.state
                .remove_partition(partition_id)
                .expect("missing removed partition");
        }

        self.pass.tick(
            &mut self.state,
            Some(update),
            |_| panic!("top-level event"),
            &mut on_info,
        );

        for event in events {
            self.state.apply(event, &mut on_info);
        }

        for (partition_id, complete) in completes {
            let old = self.state.insert_partition(partition_id, complete);
            assert!(old.is_none(), "complete replaced existing partition");
        }

        let mut checksum = S::Checksum::default();
        self.state.visit_partition_ids(|partition_id| {
            checksum.accumulate(partition_id, &self.state);
        });
        if checksum != expected_checksum {
            panic!("desync: {}", checksum.diff(&expected_checksum))
        }
    }
}

pub struct Server<S: State> {
    /// Dispatched events that must be echoed to clients.
    pending: Vec<S::Event>,
    removed: HashSet<S::PartitionId>,
}

impl<S: State> Default for Server<S> {
    fn default() -> Self {
        Self {
            pending: Vec::new(),
            removed: HashSet::new(),
        }
    }
}

#[derive(Default)]
pub struct Client;

/// Helps verify the synchronization is working.
pub trait Checksum<S: State + ?Sized>: PartialEq + Default {
    /// Add a partition ot the checksum (or no-op for `()` checksum).
    /// Order should not matter.
    fn accumulate(&mut self, partition_id: S::PartitionId, state: &S);

    fn diff(&self, server: &Self) -> String;
}

impl<S: State + ?Sized> Checksum<S> for () {
    fn accumulate(&mut self, _: S::PartitionId, _: &S) {
        // No-op
    }

    fn diff(&self, _: &Self) -> String {
        String::new()
    }
}

impl<S: State + ?Sized> Checksum<S> for u32 {
    fn accumulate(&mut self, partition_id: S::PartitionId, state: &S) {
        let mut hasher = CompatHasher::default();
        partition_id.hash(&mut hasher);
        state.hash_partition(partition_id, &mut hasher);
        *self ^= hasher.finish() as u32
    }

    fn diff(&self, server: &Self) -> String {
        format!("client: {self:?} server: {server:?}")
    }
}

impl<S: State + ?Sized> Checksum<S> for BTreeMap<S::PartitionId, S::Partition>
where
    S::PartitionId: Debug,
    S::Partition: PartialEq + Debug,
{
    fn accumulate(&mut self, partition_id: S::PartitionId, state: &S) {
        self.insert(
            partition_id,
            state
                .get_partition(partition_id)
                .expect("missing partition in checksum"),
        );
    }

    fn diff(&self, server: &Self) -> String {
        use std::fmt::Write;
        let mut ret = String::new();
        let s = &mut ret;

        for (client_k, client_v) in self.iter() {
            if let Some(server_v) = server.get(client_k) {
                if client_v != server_v {
                    writeln!(s, "{client_k:?} client: {client_v:?} server: {server_v:?}").unwrap()
                }
            } else {
                writeln!(s, "server missing {client_k:?}").unwrap();
            }
        }
        for server_k in server.keys() {
            if !self.contains_key(server_k) {
                writeln!(s, "client missing {server_k:?}").unwrap();
            }
        }
        ret
    }
}

#[cfg(test)]
mod tests {
    use crate::actor::{Client, ClientData, PassContext, PassDef, Server, State, World};
    use rand::prelude::IteratorRandom;
    use rand::{thread_rng, Rng};
    use std::cmp::Ordering;
    use std::collections::{BTreeMap, HashMap};

    #[test]
    fn fuzz() {
        #[derive(Default)]
        struct SimpleState {
            partitions: HashMap<u8, String>,
        }

        struct SimplePass;

        impl PassDef for SimplePass {
            type State = SimpleState;
            type Event = SimplePassEvent;
            type OutputEvent = SimplePassEvent;

            fn cmp(a: &Self::Event, b: &Self::Event) -> Ordering {
                fn prioritize(event: &SimplePassEvent) -> usize {
                    match event {
                        SimplePassEvent::PushChar { .. } => 0,
                        SimplePassEvent::PopChar { .. } => 0,
                        SimplePassEvent::Overwrite { string, .. } => string.len().saturating_add(1),
                    }
                }
                prioritize(a).cmp(&prioritize(b))
            }

            fn source_partition_ids(event: &Self::Event) -> Self::SourcePartitionIds {
                std::iter::once(match event {
                    SimplePassEvent::PushChar {
                        source_partition_id,
                        ..
                    } => *source_partition_id,
                    SimplePassEvent::PopChar { partition_id, .. }
                    | SimplePassEvent::Overwrite { partition_id, .. } => *partition_id,
                })
            }

            fn destination_partition_id(
                event: &Self::Event,
            ) -> <SimpleState as State>::PartitionId {
                match event {
                    SimplePassEvent::PushChar {
                        destination_partition_id,
                        ..
                    } => *destination_partition_id,
                    SimplePassEvent::PopChar { partition_id, .. }
                    | SimplePassEvent::Overwrite { partition_id, .. } => *partition_id,
                }
            }

            fn tick(
                state: &mut Self::State,
                mut on_event: impl FnMut(Self::Event),
                mut on_info: impl FnMut(<SimpleState as State>::Info<'_>),
            ) {
                for (&partition_id, string) in &mut state.partitions {
                    if string.len() % 3 == 0 {
                        string.push('m');
                        on_info(SimpleInfo::CharPushed {
                            partition_id,
                            c: 'm',
                            new: string.clone(),
                        });
                    } else {
                        on_info(SimpleInfo::CharPopped {
                            partition_id,
                            c: string.pop(),
                            new: string.clone(),
                        });
                    }

                    if partition_id % 4 == 0 {
                        on_event(SimplePassEvent::Overwrite {
                            partition_id,
                            string: String::from("ABCDE"),
                        });
                    }
                    if partition_id % 8 == 0 {
                        on_event(SimplePassEvent::Overwrite {
                            partition_id,
                            string: String::from("________"),
                        });
                    }
                    if partition_id % 3 == 0 {
                        on_event(SimplePassEvent::PushChar {
                            source_partition_id: partition_id,
                            destination_partition_id: partition_id,
                            c: 'a',
                        });
                    } else if string.len() % 7 == 0 {
                        on_event(SimplePassEvent::Overwrite {
                            partition_id,
                            string: String::from("abcd"),
                        });
                    } else {
                        on_event(SimplePassEvent::PopChar { partition_id });
                        /*
                        on_event(SimplePassEvent::PushChar {
                            source_partition_id: partition_id,
                            destination_partition_id: partition_id.saturating_sub(1),
                            c: 'b',
                        });
                        */
                    }
                }
            }

            fn apply(
                state: &mut SimpleState,
                events: impl IntoIterator<Item = Self::Event>,
                _: impl FnMut(Self::OutputEvent),
                mut on_info: impl FnMut(<Self::State as State>::Info<'_>),
            ) {
                for event in events {
                    match event {
                        SimplePassEvent::PushChar {
                            destination_partition_id,
                            c,
                            ..
                        } => {
                            let Some(partition) = state.partitions.get_mut(&destination_partition_id) else {
                                return;
                            };
                            partition.push(c);
                            on_info(SimpleInfo::CharPushed {
                                partition_id: destination_partition_id,
                                c,
                                new: partition.clone(),
                            })
                        }
                        SimplePassEvent::PopChar { partition_id } => {
                            let Some(partition) = state.partitions.get_mut(&partition_id) else {
                                return;
                            };
                            on_info(SimpleInfo::CharPopped {
                                partition_id,
                                c: partition.pop(),
                                new: partition.clone(),
                            });
                        }
                        SimplePassEvent::Overwrite {
                            partition_id,
                            string,
                        } => {
                            let Some(partition) = state.partitions.get_mut(&partition_id) else {
                                return;
                            };
                            *partition = string;
                            on_info(SimpleInfo::Overwritten {
                                partition_id,
                                new: partition.clone(),
                            });
                        }
                    }
                }
            }
        }

        #[derive(Clone, Debug)]
        enum SimpleStateEvent {
            PushChar { partition_id: u8, c: char },
        }

        #[derive(Clone, Debug)]
        enum SimplePassEvent {
            PushChar {
                source_partition_id: u8,
                destination_partition_id: u8,
                c: char,
            },
            PopChar {
                partition_id: u8,
            },
            Overwrite {
                partition_id: u8,
                string: String,
            },
        }

        #[derive(Debug)]
        #[allow(unused)]
        enum SimpleInfo {
            CharPushed {
                partition_id: u8,
                c: char,
                new: String,
            },
            CharPopped {
                partition_id: u8,
                c: Option<char>,
                new: String,
            },
            Overwritten {
                partition_id: u8,
                new: String,
            },
        }

        impl State for SimpleState {
            type PartitionId = u8;
            type Partition = String;
            type Info<'a> = SimpleInfo;
            type Event = SimpleStateEvent;
            type Checksum = BTreeMap<Self::PartitionId, Self::Partition>;

            fn destination_partition_id(event: &Self::Event) -> Self::PartitionId {
                match event {
                    SimpleStateEvent::PushChar { partition_id, .. } => *partition_id,
                }
            }

            fn visit_partition_ids(&self, visitor: impl FnMut(Self::PartitionId)) {
                self.partitions.keys().copied().for_each(visitor);
            }

            fn get_partition(&self, partition_id: Self::PartitionId) -> Option<Self::Partition> {
                self.partitions.get(&partition_id).cloned()
            }

            fn insert_partition(
                &mut self,
                partition_id: Self::PartitionId,
                partition: Self::Partition,
            ) -> Option<Self::Partition> {
                self.partitions.insert(partition_id, partition)
            }

            fn remove_partition(
                &mut self,
                partition_id: Self::PartitionId,
            ) -> Option<Self::Partition> {
                self.partitions.remove(&partition_id)
            }

            fn apply(&mut self, event: Self::Event, mut on_info: impl FnMut(Self::Info<'_>)) {
                match event {
                    SimpleStateEvent::PushChar { partition_id, c } => {
                        let partition = self.partitions.get_mut(&partition_id).unwrap();
                        partition.push(c);
                        on_info(SimpleInfo::CharPushed {
                            partition_id,
                            c,
                            new: partition.clone(),
                        });
                    }
                }
            }
        }

        type SimplePasses =
            PassContext<SimpleState, SimplePass, PassContext<SimpleState, SimplePass>>;

        #[derive(Default)]
        struct MockClient {
            world: World<SimpleState, SimplePasses, Client>,
            data: ClientData<SimpleState>,
        }

        const DEBUG: bool = false;
        fn on_info(info: SimpleInfo) {
            if DEBUG {
                println!("Info: {:?}", info);
            }
        }

        let update_clients =
            |server: &mut World<SimpleState, SimplePasses, Server<SimpleState>>,
             clients: &mut [MockClient]| {
                let n_clients = clients.len();
                let mut rng = thread_rng();
                for (i, client) in clients.iter_mut().enumerate() {
                    let visibility = server
                        .partitions
                        .keys()
                        .copied()
                        .filter(|&n| {
                            rng.gen_bool(if n as usize % n_clients == i {
                                0.9
                            } else {
                                0.1
                            })
                        })
                        .collect::<Vec<_>>();
                    let update = server.update(&mut client.data, visibility);
                    let has = &client.world.partitions;
                    if DEBUG {
                        println!("{i} has {has:?} gets {update:?}");
                    }
                    client.world.tick(update, on_info);
                }
            };

        let mut rng = thread_rng();
        let isolate = false;

        for i in 0..512 {
            if DEBUG {
                println!("@@@@@@@@@@@@@@@@@@@@@@@@ FUZZ #{i}");
            }

            let mut server = World::<SimpleState, SimplePasses, Server<SimpleState>>::default();
            let mut clients = std::iter::repeat_with(MockClient::default)
                .take(if isolate { 1 } else { rng.gen_range(0..=32) })
                .collect::<Vec<_>>();

            let mut possible_partitions = if isolate {
                vec![22, 23]
            } else {
                (0..32).collect::<Vec<_>>()
            };

            for j in 0..rng.gen_range(1..=16) {
                if DEBUG {
                    println!("@@@@@@@@@@@@@@@ ITERATION #{j}");
                    println!("@@@@@@@ DISPATCH");
                }

                for _ in 0..rng.gen_range(0..=4) {
                    if !possible_partitions.is_empty() {
                        let i = rng.gen_range(0..possible_partitions.len());
                        let partition_id = possible_partitions.swap_remove(i);
                        server.insert_partition(partition_id, i.to_string());
                    }
                }

                if DEBUG {
                    println!("@@@@@@@ DISPATCH 2");
                }

                if !server.partitions.is_empty() {
                    for _ in 0..rng.gen_range(0..=if isolate { 3 } else { 25 }) {
                        server.dispatch(
                            SimpleStateEvent::PushChar {
                                partition_id: *server.partitions.keys().choose(&mut rng).unwrap(),
                                c: rng.gen_range('0'..='9'),
                            },
                            on_info,
                        );
                    }
                }

                if DEBUG {
                    println!("@@@@@@@ UPDATE CLIENTS");
                }

                update_clients(&mut server, &mut clients);

                if DEBUG {
                    println!("@@@@@@@ TICK: {:?}", server.state.partitions);
                }

                server.tick(on_info);
            }
        }
    }
}
