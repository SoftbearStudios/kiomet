// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(extract_if)]
#![feature(extend_one)]
#![feature(impl_trait_in_assoc_type)]
#![feature(int_roundings)]
#![feature(lazy_cell)]
#![feature(let_chains)]
#![feature(option_get_or_insert_default)]
#![feature(test)]
#![feature(variant_count)]
#![feature(vec_push_within_capacity)]

use kodiak_common::{DefaultedGameConstants, GameConstants};

// Actually required see https://doc.rust-lang.org/beta/unstable-book/library-features/test.html
#[cfg(test)]
extern crate core;
#[cfg(test)]
extern crate test;

mod combatants;
#[macro_use]
mod macros;

pub mod alerts;
pub mod chunk;
pub mod death_reason;
pub mod enum_array;
pub mod field;
pub mod force;
pub mod info;
pub mod player;
pub mod protocol;
pub mod singleton;
pub mod ticks;
pub mod tower;
pub mod unit;
pub mod units;
pub mod world;

pub const KIOMET_CONSTANTS: &'static GameConstants = &GameConstants {
    domain: "kiomet.com",
    game_id: "Kiomet",
    geodns_enabled: true,
    name: "Kiomet",
    trademark: "Kiomet",
    server_names: &["Asgard", "Camelot", "Olympus", "Svarga", "Valhalla"],
    defaulted: DefaultedGameConstants::new(),
};

// Save memory.
pub(crate) fn shrink_vec<T>(v: &mut Vec<T>) {
    if v.is_empty() || v.capacity() > v.len() * 2 + 2 {
        v.shrink_to_fit();
    }
}
