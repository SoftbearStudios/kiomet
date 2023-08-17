// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

// Don't use Arcs on client.
// Certain things like MessageDto are deduplicated on the server.

// Owned is primarily used as Owned<[T]>.
// Box and Arc serialize the same way so this works.
#[cfg(feature = "server")]
pub type Owned<T> = std::sync::Arc<T>;
#[cfg(not(feature = "server"))]
pub type Owned<T> = Box<T>;

// TODO make Owned a struct and make these free functions methods.
pub fn owned_into_box<T: Clone + 'static>(owned: Owned<[T]>) -> Box<[T]> {
    #[cfg(feature = "server")]
    return owned.iter().cloned().collect();
    #[cfg(not(feature = "server"))]
    owned
}

pub fn owned_into_iter<T: Clone + 'static>(owned: Owned<[T]>) -> impl Iterator<Item = T> {
    Vec::from(owned_into_box(owned)).into_iter()
}

// Dedup is used as Dedup<Expensive>.
// Take care to ensure T is sized.
// Arc<T> and T serialize the same way as long as T is sized.
#[cfg(feature = "server")]
pub type Dedup<T> = std::sync::Arc<T>;
#[cfg(not(feature = "server"))]
pub type Dedup<T> = T;

pub fn dedup_into_inner<T: Clone>(t: Dedup<T>) -> T {
    #[cfg(feature = "server")]
    return (*t).clone();
    #[cfg(not(feature = "server"))]
    t
}
