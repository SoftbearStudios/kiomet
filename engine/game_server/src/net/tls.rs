use axum_server::tls_rustls::RustlsConfig;
use log::{error, warn};
use rustls::server::ServerConfig;
use server_util::ssl::config_from_pem;
use std::{
    fs::File,
    future::Future,
    io::{self, BufReader, Cursor},
    sync::Arc,
    time::Duration,
};

pub async fn rustls_config(
    certificate_private_key_paths: Option<(Arc<str>, Arc<str>)>,
) -> RustlsConfig {
    let self_signed = config_from_pem(
        &mut Cursor::new(&include_bytes!("certificate.pem")),
        &mut Cursor::new(&include_bytes!("private_key.pem")),
    )
    .unwrap();

    if let Some((certificate_path, private_key_path)) = certificate_private_key_paths {
        let rustls_config = RustlsConfig::from_config(
            load_server_config(&certificate_path, &private_key_path)
                .await
                .unwrap_or_else(|e| {
                    error!("Could not load certificate: {e:?}");
                    Arc::clone(&self_signed)
                }),
        );

        {
            let rustls_config = rustls_config.clone();

            tokio::spawn(async move {
                let mut governor = tokio::time::interval(Duration::from_secs(6 * 60 * 60));

                loop {
                    governor.tick().await;

                    warn!("renewing SSL certificate...");
                    let result = {
                        let certificate_path = Arc::clone(&certificate_path);
                        let private_key_path = Arc::clone(&private_key_path);
                        tokio::task::spawn_blocking(move || {
                            config_from_pem(
                                &mut BufReader::new(File::open(certificate_path.as_ref())?),
                                &mut BufReader::new(File::open(private_key_path.as_ref())?),
                            )
                        })
                        .await
                        .unwrap()
                    };

                    match result {
                        Ok(config) => {
                            rustls_config.reload_from_config(config);
                        }
                        Err(e) => {
                            error!("failed to renew SSL certificate: {}", e);
                        }
                    }
                }
            });
        }

        rustls_config
    } else {
        RustlsConfig::from_config(self_signed)
    }
}

fn load_server_config(
    cert: &Arc<str>,
    key: &Arc<str>,
) -> impl Future<Output = io::Result<Arc<ServerConfig>>> + 'static {
    let cert = Arc::clone(&cert);
    let key = Arc::clone(&key);
    async move {
        tokio::task::spawn_blocking(move || {
            config_from_pem(
                &mut BufReader::new(File::open(cert.as_ref())?),
                &mut BufReader::new(File::open(key.as_ref())?),
            )
        })
        .await?
    }
}
