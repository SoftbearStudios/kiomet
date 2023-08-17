// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::visible::Visible;
use client_util::apply::Apply;
use common::info::InfoEvent;
use common::protocol::{Diff, NonActor, Update};
use common::ticks::Ticks;
use common::world::{ApplyOwned, World};
use std::ops::Deref;

#[derive(Default)]
pub struct TowerState {
    non_actor: NonActor,
    pub world: World,
    pub visible: Visible,
    pub info_events: Vec<InfoEvent>,
    /// In seconds; for interpolation.
    pub time_since_last_tick: f32,
    pub ticked: bool, // Consumed in update.
}

impl Deref for TowerState {
    type Target = NonActor;

    fn deref(&self) -> &Self::Target {
        &self.non_actor
    }
}

impl Apply<Update> for TowerState {
    fn apply(&mut self, update: Update) {
        self.non_actor.apply(&update.non_actor_diff);

        let mut on_info_event = |info_event| {
            if self.info_events.len() < 128 {
                self.info_events.push(info_event);
            }
        };

        // js_hooks::console_log!("{:?}", update);
        self.world
            .apply_owned(update.actor_update, &mut on_info_event);

        // Last tick is now.
        // Could set to zero, but this will more gradually account for jitter.
        self.time_since_last_tick =
            (self.time_since_last_tick - Ticks::PERIOD_SECS).clamp(-1.0, 1.0) * 0.6;

        // Invalidate visible cache.
        self.visible.ticked();

        // Set ticked to true to be taken in update.
        self.ticked = true;
    }
}
