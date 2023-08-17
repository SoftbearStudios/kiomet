// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use std::fs;
use std::path::Path;

const CERTIFICATE_PATH: &str = "./src/net/certificate.pem";
const PRIVATE_KEY_PATH: &str = "./src/net/private_key.pem";

fn main() {
    if !(Path::new(CERTIFICATE_PATH).exists() && Path::new(PRIVATE_KEY_PATH).exists()) {
        let mut names = vec!["localhost".to_owned()];
        if let Ok(hostname) = gethostname::gethostname().into_string() {
            names.push(hostname);
        }
        let mut params = rcgen::CertificateParams::new(names);
        params.is_ca = rcgen::IsCa::Ca(rcgen::BasicConstraints::Constrained(2));
        let cert = rcgen::Certificate::from_params(params).unwrap();
        fs::write(CERTIFICATE_PATH, cert.serialize_pem().unwrap().into_bytes()).unwrap();
        fs::write(
            PRIVATE_KEY_PATH,
            cert.serialize_private_key_pem().into_bytes(),
        )
        .unwrap();
    }
}
