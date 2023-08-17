// SPDX-FileCopyrightText: 2021 Softbear, Inc.
// SPDX-License-Identifier: AGPL-3.0-or-later

use core_protocol::UnixTime;
#[allow(unused)]
use rustls::{server::AllowAnyAnonymousOrAuthenticatedClient, OwnedTrustAnchor};
#[allow(unused)]
use rustls::{Certificate, PrivateKey, RootCertStore, ServerConfig};
use rustls_pemfile::Item;
use std::io::BufRead;
use std::sync::Arc;
use std::{
    fs::File,
    io::{self, ErrorKind, Read},
};
use x509_parser::prelude::parse_x509_pem;

pub fn certificate_expiry(certificate_file: &str) -> Result<UnixTime, String> {
    let mut f = File::open(certificate_file).map_err(|e| e.to_string())?;
    let mut cert = Vec::new();
    f.read_to_end(&mut cert).map_err(|e| e.to_string())?;
    let (_, pem) = parse_x509_pem(&cert).map_err(|e| e.to_string())?;
    let x509 = pem.parse_x509().map_err(|e| e.to_string())?;
    Ok(x509.validity().not_after.timestamp() as UnixTime)
}

pub fn config_from_pem(
    cert: &mut dyn BufRead,
    key: &mut dyn BufRead,
) -> io::Result<Arc<ServerConfig>> {
    let cert = rustls_pemfile::certs(cert)?;
    let item = rustls_pemfile::read_one(key)?;
    let key = match item {
        Some(Item::RSAKey(key) | Item::PKCS8Key(key) | Item::ECKey(key)) => key,
        _ => {
            return Err(io::Error::new(
                ErrorKind::Other,
                "private key format not supported",
            ))
        }
    };

    config_from_der(cert, key).map(Arc::new)
}

fn config_from_der(cert: Vec<Vec<u8>>, key: Vec<u8>) -> io::Result<ServerConfig> {
    let cert = cert.into_iter().map(Certificate).collect();
    let key = PrivateKey(key);

    /*
    let mut roots = RootCertStore::empty();
    roots.add_server_trust_anchors(webpki_roots::TLS_SERVER_ROOTS.0.iter().map(|ta| {
        OwnedTrustAnchor::from_subject_spki_name_constraints(
            ta.subject,
            ta.spki,
            ta.name_constraints,
        )
    }));
    */

    let mut config = ServerConfig::builder()
        .with_safe_defaults()
        .with_no_client_auth()
        //.with_client_cert_verifier(AllowAnyAnonymousOrAuthenticatedClient::new(roots))
        .with_single_cert(cert, key)
        .map_err(|e| io::Error::new(ErrorKind::Other, e))?;

    config.alpn_protocols = vec![b"h2".to_vec(), b"http/1.1".to_vec()];

    Ok(config)
}
