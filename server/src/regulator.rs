// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::PlayerId;
use std::collections::{hash_map::Entry, HashMap};

const LOG: bool = false;

#[derive(PartialEq, Eq, Copy, Clone, Debug)]
enum Tense {
    /// Expired: "is has happened some time in the past"
    Past,
    /// Current: "it is happening as soon as possible"
    Present,
    /// Not yet: "it could happen"
    Future,
}

#[allow(unused)]
impl Tense {
    fn is_past(&self) -> bool {
        matches!(self, Self::Past)
    }

    fn is_present(&self) -> bool {
        matches!(self, Self::Present)
    }

    fn is_future(&self) -> bool {
        matches!(self, Self::Future)
    }
}

struct State {
    join: Tense,
    leave: Tense,
}

#[derive(Default)]
pub struct Regulator {
    states: HashMap<PlayerId, State>,
}

impl Regulator {
    /// Returns `true` if the join is fast-path.
    #[must_use = "fast path must exist"]
    pub fn join(&mut self, player_id: PlayerId) -> bool {
        match self.states.entry(player_id) {
            Entry::Vacant(vacant) => {
                if LOG {
                    println!("{player_id:?} joined on fast path");
                }
                vacant.insert(State {
                    join: Tense::Past,
                    leave: Tense::Future,
                });
                true
            }
            Entry::Occupied(mut occupied) => {
                let state = occupied.get_mut();
                debug_assert_eq!(state.join, Tense::Future, "already joining/joined");
                state.join = Tense::Present;
                false
            }
        }
    }

    pub fn leave(&mut self, player_id: PlayerId) {
        if let Some(state) = self.states.get_mut(&player_id) {
            debug_assert_ne!(state.join, Tense::Future, "not joining/joined");
            state.join = Tense::Future;
            if state.leave.is_future() {
                state.leave = Tense::Present;
            }
        } else {
            debug_assert!(false, "not joining/joined");
        }
    }

    pub fn active(&self, player_id: PlayerId) -> bool {
        self.states
            .get(&player_id)
            .map(|state| state.join.is_past() && state.leave.is_future())
            .unwrap_or(false)
    }

    pub fn tick(&mut self, mut add_remove: impl FnMut(PlayerId, bool)) {
        self.states.retain(|&player_id, state| {
            match state.leave {
                Tense::Past => {
                    // Left (always possible).
                    if LOG {
                        println!("{player_id} left");
                    }
                    add_remove(player_id, false);

                    if state.join.is_past() {
                        state.join = Tense::Future;
                    }
                    state.leave = Tense::Future;
                    true
                }
                Tense::Present => {
                    // Leaving.
                    if LOG {
                        println!("{player_id} leaving");
                    }
                    state.leave = Tense::Past;
                    true
                }
                Tense::Future => match state.join {
                    Tense::Past => {
                        // Steady-state.
                        true
                    }
                    Tense::Present => {
                        // Joined.
                        if LOG {
                            println!("{player_id} joined");
                        }
                        add_remove(player_id, true);
                        state.join = Tense::Past;
                        state.leave = Tense::Future;
                        true
                    }
                    Tense::Future => {
                        if LOG {
                            println!("{player_id} expired");
                        }
                        // Default `State`, so doesn't belong in map.
                        false
                    }
                },
            }
        });
    }
}
