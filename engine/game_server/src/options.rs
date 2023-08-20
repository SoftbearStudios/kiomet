// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::id::RegionId;
use log::LevelFilter;
use std::{net::IpAddr, sync::Arc};
use structopt::StructOpt;

/// Server options, to be specified as arguments.
#[derive(Debug, StructOpt)]
pub struct Options {
    /// Minimum number of bots.
    #[structopt(long)]
    pub min_bots: Option<usize>,
    /// Maximum number of bots.
    #[structopt(long)]
    pub max_bots: Option<usize>,
    /// This percent of real players will help determine number of bots.
    #[structopt(long)]
    pub bot_percent: Option<usize>,
    /// Log incoming HTTP requests
    #[cfg_attr(debug_assertions, structopt(long, default_value = "warn"))]
    #[cfg_attr(not(debug_assertions), structopt(long, default_value = "error"))]
    pub debug_http: LevelFilter,
    /// Log game diagnostics
    #[cfg_attr(debug_assertions, structopt(long, default_value = "info"))]
    #[cfg_attr(not(debug_assertions), structopt(long, default_value = "error"))]
    pub debug_game: LevelFilter,
    /// Log game engine diagnostics
    #[cfg_attr(debug_assertions, structopt(long, default_value = "info"))]
    #[cfg_attr(not(debug_assertions), structopt(long, default_value = "warn"))]
    pub debug_engine: LevelFilter,
    /// Log plasma diagnostics
    #[cfg_attr(debug_assertions, structopt(long, default_value = "info"))]
    #[cfg_attr(not(debug_assertions), structopt(long, default_value = "info"))]
    pub debug_plasma: LevelFilter,
    /// Log chats here
    #[structopt(long)]
    pub chat_log: Option<String>,
    /// Log client traces here
    #[structopt(long)]
    pub trace_log: Option<String>,
    /// Server id.
    #[structopt(long, default_value = "0")]
    pub server_id: u8,
    #[structopt(long)]
    /// Override the server ip (currently used to detect the region).
    pub ip_address: Option<IpAddr>,
    #[structopt(long)]
    pub http_port: Option<u16>,
    #[structopt(long)]
    pub https_port: Option<u16>,
    /// Override the region id.
    #[structopt(long)]
    pub region_id: Option<RegionId>,
    /// Domain (without server id prepended).
    #[allow(dead_code)]
    #[deprecated = "now from game id"]
    #[structopt(long)]
    pub domain: Option<String>,
    /// Certificate chain path.
    #[structopt(long)]
    pub certificate_path: Option<String>,
    /// Private key path.
    #[structopt(long)]
    pub private_key_path: Option<String>,
    /// HTTP request bandwidth limiting (in bytes per second).
    #[structopt(long, default_value = "500000")]
    pub http_bandwidth_limit: u32,
    /// HTTP request rate limiting burst (in bytes).
    ///
    /// Implicit minimum is double the total size of the client static files.
    #[structopt(long)]
    pub http_bandwidth_burst: Option<u32>,
    /// Client authenticate rate limiting period (in seconds).
    #[structopt(long, default_value = "30")]
    pub client_authenticate_rate_limit: u64,
    /// Client authenticate rate limiting burst.
    #[structopt(long, default_value = "16")]
    pub client_authenticate_burst: u32,
}

impl Options {
    pub(crate) fn certificate_private_key_paths(&self) -> Option<(Arc<str>, Arc<str>)> {
        self.certificate_path
            .as_deref()
            .zip(self.private_key_path.as_deref())
            .map(|(c, p)| (c.into(), p.into()))
    }

    pub(crate) fn bandwidth_burst(&self, static_size: usize) -> u32 {
        self.http_bandwidth_burst.unwrap_or(static_size as u32 * 2)
    }

    pub(crate) const STANDARD_HTTP_PORT: u16 = 80;
    pub(crate) const STANDARD_HTTPS_PORT: u16 = 443;

    pub(crate) fn http_and_https_ports(&self) -> (u16, u16) {
        #[cfg(unix)]
        let priviledged = nix::unistd::Uid::effective().is_root();

        #[cfg(not(unix))]
        let priviledged = true;

        let (http_port, https_port) = if priviledged {
            (Self::STANDARD_HTTP_PORT, Self::STANDARD_HTTPS_PORT)
        } else {
            (8080, 8443)
        };

        let ports = (
            self.http_port.unwrap_or(http_port),
            self.https_port.unwrap_or(https_port),
        );
        log::info!("HTTP port: {}, HTTPS port: {}", ports.0, ports.1);
        ports
    }
}
