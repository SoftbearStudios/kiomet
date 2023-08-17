// SPDX-FileCopyrightText: 2023 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(entry_insert)]
#![feature(int_roundings)]
#![feature(let_chains)]
#![feature(type_alias_impl_trait)]

use service::TowerService;

mod bot;
mod regulator;
mod service;
mod world;

fn main() {
    let cdn = minicdn::release_include_mini_cdn!("../../client/dist/");
    game_server::entry_point::entry_point::<TowerService>(cdn, true);
}
