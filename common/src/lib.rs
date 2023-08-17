// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(array_methods)]
#![feature(drain_filter)]
#![feature(extend_one)]
#![feature(impl_trait_in_assoc_type)]
#![feature(int_roundings)]
#![feature(lazy_cell)]
#![feature(let_chains)]
#![feature(option_get_or_insert_default)]
#![feature(test)]
#![feature(variant_count)]
#![feature(vec_push_within_capacity)]

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

// Save memory.
pub(crate) fn shrink_vec<T>(v: &mut Vec<T>) {
    if v.is_empty() || v.capacity() > v.len() * 2 + 2 {
        v.shrink_to_fit();
    }
}
