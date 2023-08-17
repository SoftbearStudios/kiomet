// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::rpc::AdminRequest;
use minicdn::EmbeddedMiniCdn;
use std::time::Duration;
use structopt::StructOpt;

#[derive(StructOpt)]
struct Options {
    #[structopt(long)]
    auth: String,
    #[structopt(long)]
    client_path: Option<String>,
    #[structopt(long)]
    rustrict_trie: bool,
    #[structopt(long)]
    rustrict_replacements: bool,
    #[structopt(long)]
    url: String,
    #[structopt(long)]
    no_compress: bool,
}

fn main() {
    let options: Options = Options::from_args();

    upload_client(&options);
    upload_rustrict_trie(&options);
    upload_rustrict_replacements(&options);
}

fn post(options: &Options, request: AdminRequest) {
    let body = serde_json::to_string(&request).unwrap();

    eprintln!();
    eprintln!("total bytes: {}", body.len());

    eprintln!();
    eprintln!("pausing for a moment...");

    std::thread::sleep(Duration::from_secs(2));

    eprintln!("uploading...");

    let client = reqwest::blocking::ClientBuilder::new()
        .tcp_keepalive(Some(Duration::from_secs(10)))
        .pool_max_idle_per_host(0)
        .timeout(Duration::from_secs(360))
        .build()
        .unwrap();

    match client
        .post(&options.url)
        .bearer_auth(&options.auth)
        .header("content-type", "application/json")
        .body(body)
        .send()
    {
        Ok(response) => {
            let status = response.status();
            match response.text() {
                Ok(text) => {
                    println!("received: {} (code {})", text, status);
                }
                Err(e) => eprintln!("{}", e.to_string()),
            }
        }
        Err(e) => eprintln!("{}", e.to_string()),
    }
}

fn upload_client(options: &Options) {
    let path = match options.client_path.as_deref() {
        Some(p) => p,
        None => return,
    };
    eprintln!("preparing game client from {path}...");

    let cdn = if options.no_compress {
        EmbeddedMiniCdn::new(path)
    } else {
        EmbeddedMiniCdn::new_compressed(path)
    };

    let len = cdn.iter().count();

    if len == 0 {
        eprintln!("no files");
        std::process::exit(1);
    }

    eprintln!("found {} files", len);

    for (path, file) in cdn.iter() {
        eprintln!(
            " - \"{}\" ({} bytes uncompressed)",
            path,
            file.contents.len()
        );
    }
    post(options, AdminRequest::SetGameClient(cdn));
}

fn upload_rustrict_trie(options: &Options) {
    if options.rustrict_trie {
        println!("Uploading rustrict trie...");
        post(options, AdminRequest::SetRustrictTrie(Default::default()));
    }
}

fn upload_rustrict_replacements(options: &Options) {
    if options.rustrict_replacements {
        println!("Uploading rustrict replacements...");
        post(
            options,
            AdminRequest::SetRustrictReplacements(Default::default()),
        );
    }
}
