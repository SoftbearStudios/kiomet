// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(const_option)]
#![feature(lazy_cell)]

use std::time::{SystemTime, UNIX_EPOCH};

pub use bitcode;

pub mod dto;
pub mod id;
pub mod metrics;
pub mod name;
pub mod owned;
pub mod plasma;
pub mod prelude;
pub mod rpc;
pub mod serde_util;

pub use dto::*;
pub use id::*;
pub use metrics::*;
pub use name::*;
pub use plasma::*;
pub use rpc::*;
pub use serde_util::{is_default, StrVisitor};

pub type UnixTime = u64;

pub fn get_unix_time_now() -> UnixTime {
    match SystemTime::now().duration_since(UNIX_EPOCH) {
        Ok(duration) => duration.as_millis() as u64,
        _ => 0,
    }
}
