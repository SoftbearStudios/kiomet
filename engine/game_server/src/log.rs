// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use crate::options::Options;

pub(crate) fn init_logger(options: &Options) {
    let mut logger = env_logger::builder();
    logger.format_timestamp(None);
    logger.filter_module("server", options.debug_game);
    logger.filter_module("game_server", options.debug_engine);
    logger.filter_module("game_server::plasma", options.debug_plasma);
    logger.filter_module("game_server::entry_point", options.debug_http);
    logger.init();
}
