// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::enum_array::EnumArray;
use num_enum::{IntoPrimitive, TryFromPrimitive};
use strum::{EnumIter, IntoEnumIterator};

/// Fields ordered by distance above ground.
#[derive(
    Copy,
    Clone,
    Eq,
    PartialEq,
    Ord,
    PartialOrd,
    Hash,
    Debug,
    EnumIter,
    IntoPrimitive,
    TryFromPrimitive,
)]
#[repr(u8)]
pub enum Field {
    Surface,
    Air,
}

pub type FieldArray<V> = EnumArray<Field, V, { std::mem::variant_count::<Field>() }>;

impl Field {
    pub fn iter() -> impl Iterator<Item = Self> + 'static {
        <Self as IntoEnumIterator>::iter()
    }
}
