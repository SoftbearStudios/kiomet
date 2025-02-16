// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::ticks::Ticks;
use crate::world::Apply;
use kodiak_common::actor_model::*;
use kodiak_common::bitcode::{self, *};

#[derive(Copy, Clone, Debug, Hash, PartialEq, PartialOrd, Encode, Decode)]
pub struct SingletonId;

impl ActorId for SingletonId {
    type SparseMap<T> = Option<(Self, T)>;
    type Map<T> = Option<(Self, T)>;
}

#[derive(Clone, Debug, Default, Hash, Encode, Decode)]
pub struct Singleton {
    pub tick: Ticks,
}

impl Actor for Singleton {
    type Id = SingletonId;
}

#[derive(Clone, Debug, Encode, Decode)]
pub enum SingletonInput {}

impl Message for SingletonInput {}

impl<C> Apply<SingletonInput, C> for Singleton {
    fn apply(&mut self, _: &SingletonInput, _: &mut C) {}
}
