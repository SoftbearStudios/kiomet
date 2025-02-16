// SPDX-FileCopyrightText: 2024 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

#![feature(int_roundings)]
#![feature(iter_intersperse)]
#![feature(let_chains)]
#![feature(option_get_or_insert_default)]
#![feature(stmt_expr_attributes)]
#![feature(variant_count)]

use crate::ui::KiometRoute;
use game::KiometGame;
use kodiak_client::GameClient;

mod animation;
mod background;
mod color;
mod finite_index;
mod game;
mod key_dispenser;
mod layout;
mod path;
mod road;
mod settings;
mod state;
mod territory;
mod tutorial;
mod ui;
mod visible;

fn main() {
    KiometGame::run();
}
