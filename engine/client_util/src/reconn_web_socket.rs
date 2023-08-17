// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::apply::Apply;
use crate::web_socket::{ProtoWebSocket, State};
use core_protocol::prelude::*;
use std::marker::PhantomData;

/// Reconnectable WebSocket (generic over inbound, outbound, and state).
/// Old state is preserved after closing, but cleared when a new connection is reopened.
pub struct ReconnWebSocket<I, O, S> {
    inner: ProtoWebSocket<I, O>,
    host: String,
    /// Send when opening a new socket.
    preamble: Option<O>,
    tries: u8,
    next_try: f32,
    _spooky: PhantomData<S>,
}

impl<I, O, S> ReconnWebSocket<I, O, S>
where
    I: 'static + Decode,
    O: 'static + Encode + Clone,
    S: Apply<I>,
{
    const MAX_TRIES: u8 = 5;
    const SECONDS_PER_TRY: f32 = 1.0;

    pub fn new(host: String, preamble: Option<O>) -> Self {
        let mut inner = ProtoWebSocket::new(&host);

        if let Some(p) = preamble.as_ref() {
            inner.send(p.clone());
        }

        Self {
            inner,
            preamble,
            host,
            tries: 0,
            next_try: 0.0,
            _spooky: PhantomData,
        }
    }

    /// Returns whether the underlying connection is closed (for any reason).
    pub fn is_closed(&self) -> bool {
        self.inner.is_closed()
    }

    /// Returns whether the underlying connection is open.
    pub fn is_open(&self) -> bool {
        self.inner.is_open()
    }

    pub fn is_reconnecting(&self) -> bool {
        matches!(self.inner.state(), State::Opening | State::Error)
            && (1..=Self::MAX_TRIES).contains(&self.tries)
    }

    /// Returns whether the underlying connection is closed and reconnection attempts have been
    /// exhausted.
    pub fn is_terminated(&self) -> bool {
        (self.inner.state() == State::Closed
            || (self.inner.is_error() && self.tries >= Self::MAX_TRIES))
            && !self.inner.has_updates()
    }

    /// Takes the current time, and returns a collection of updates to apply to the current
    /// state. Will automatically reconnect and clear state if/when the underlying connection is new.
    ///
    /// TODO: Until further notice, it is the caller's responsibility to apply the state changes.
    pub fn update(&mut self, state: &mut S, time_seconds: f32) -> Vec<I> {
        self.reconnect_if_necessary(state, time_seconds);
        self.inner.receive_updates()
    }

    /// Reset the host (for future connections) to a different value.
    pub fn reset_host(&mut self, host: String) {
        self.host = host;
    }

    /// Reset the preamble (for future connections) to a different value.
    pub fn reset_preamble(&mut self, preamble: O) {
        self.preamble = Some(preamble);
    }

    /// Sends a message, or queues it for sending when the underlying connection is open.
    pub fn send(&mut self, msg: O) {
        self.inner.send(msg);
    }

    /// Attempts to reestablish a connection if necessary. This does not and should not preserve
    /// pending messages.
    fn reconnect_if_necessary(&mut self, state: &mut S, time_seconds: f32) {
        if self.inner.state() == State::Open {
            if self.tries > 0 {
                // Reconnected, forget state/tries.
                js_hooks::console_log!("reconnected websocket after {} attempts.", self.tries);
                state.reset();
                self.tries = 0;
                self.next_try = time_seconds + Self::SECONDS_PER_TRY * 0.5;
            }
        } else if time_seconds < self.next_try {
            // Wait...
        } else if self.inner.is_error() && self.tries < Self::MAX_TRIES {
            // Try again.
            self.inner = ProtoWebSocket::new(&self.host);
            if let Some(p) = self.preamble.as_ref() {
                self.inner.send(p.clone());
            }
            self.tries += 1;
            self.next_try = time_seconds + Self::SECONDS_PER_TRY;
        } else if self.is_terminated() {
            // Stop trying, stop giving the impression of working.
            state.reset();
        }
    }

    /// Drop, but leave open the possibility of auto-reconnecting (useful for testing Self).
    pub fn simulate_drop(&mut self) {
        self.inner.close();
    }
}

impl<I, O, S> Drop for ReconnWebSocket<I, O, S> {
    fn drop(&mut self) {
        self.inner.close();
    }
}
