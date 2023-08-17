// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::iter::FromIterator;
use std::sync::Arc;

pub fn arc_default_n<T: Default>(n: usize) -> Arc<[T]> {
    Arc::from_iter((0..n).map(|_| T::default()))
}

pub fn box_default_n<T: Default>(n: usize) -> Box<[T]> {
    Box::from_iter((0..n).map(|_| T::default()))
}
