// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(entry_insert)]
#![feature(int_roundings)]
#![feature(let_chains)]
#![feature(type_alias_impl_trait)]

use kodiak_server::{entry_point, minicdn};
use service::TowerService;
use std::process::ExitCode;

mod bot;
mod service;
mod world;

fn main() -> ExitCode {
    let cdn = minicdn::release_include_mini_cdn!("../../client/dist/");
    entry_point::<TowerService>(cdn)
}
