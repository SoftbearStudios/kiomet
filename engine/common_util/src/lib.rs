// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(array_windows)]
#![feature(associated_type_defaults)]
#![feature(extend_one)]
#![feature(impl_trait_in_assoc_type)]
#![feature(int_roundings)]
#![feature(is_sorted)]
#![feature(never_type)]
#![feature(test)]
#![feature(vec_push_within_capacity)]
#![feature(option_get_or_insert_default)]

// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

// Actually required see https://doc.rust-lang.org/beta/unstable-book/library-features/test.html
#[cfg(test)]
extern crate core;
#[cfg(test)]
extern crate test;

// TODO actor mod.
pub mod actor2;
pub mod storage;

pub mod actor;
pub mod alloc;
pub mod angle;
pub mod collision;
pub mod entities;
pub mod hash;
pub mod mask;
pub mod range;
pub mod regulator;
pub mod sector;
pub mod serde;
pub mod ticks;
pub mod x_vec2;
