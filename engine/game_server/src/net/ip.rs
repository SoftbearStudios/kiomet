use core_protocol::RegionId;
use futures::{stream::FuturesUnordered, StreamExt};
use hyper::{http::HeaderValue, HeaderMap};
use log::{info, warn};
use reqwest::Client;
use std::{collections::HashMap, net::IpAddr, str::FromStr, time::Duration};

pub fn ip_to_region_id(ip: IpAddr) -> Option<RegionId> {
    use db_ip::{include_region_database, DbIpDatabase, Region};

    lazy_static::lazy_static! {
        static ref DB_IP: DbIpDatabase<Region> = include_region_database!();
    }

    /// Convert from [`db_ip::Region`] to [`core_protocol::id::RegionId`].
    /// The mapping is one-to-one, since the types mirror each other.
    fn region_to_region_id(region: Region) -> RegionId {
        match region {
            Region::Africa => RegionId::Africa,
            Region::Asia => RegionId::Asia,
            Region::Europe => RegionId::Europe,
            Region::NorthAmerica => RegionId::NorthAmerica,
            Region::Oceania => RegionId::Oceania,
            Region::SouthAmerica => RegionId::SouthAmerica,
        }
    }

    DB_IP.get(&ip).map(region_to_region_id)
}

/// Gets public ip by consulting various 3rd party APIs.
pub async fn get_own_public_ip() -> Option<IpAddr> {
    let mut default_headers = HeaderMap::new();

    default_headers.insert(
        reqwest::header::CONNECTION,
        HeaderValue::from_str("close").unwrap(),
    );

    let client = Client::builder()
        .timeout(Duration::from_secs(1))
        .http1_only()
        .default_headers(default_headers)
        .build()
        .ok()?;

    let checkers = [
        "https://v4.ident.me/",
        "https://v4.tnedi.me/",
        "https://ipecho.net/plain",
        "https://ifconfig.me/ip",
        "https://icanhazip.com/",
        "https://ipinfo.io/ip",
        "https://api.ipify.org/",
    ];

    let mut checks: FuturesUnordered<_> = checkers
        .iter()
        .map(move |&checker| {
            let client = client.clone();
            let request_result = client.get(checker).build();

            async move {
                let request = request_result.ok()?;
                let fut = client.execute(request);

                let response = match fut.await {
                    Ok(response) => response,
                    Err(e) => {
                        info!("checker {} returned {:?}", checker, e);
                        return None;
                    }
                };

                let string = match response.text().await {
                    Ok(string) => string,
                    Err(e) => {
                        info!("checker {} returned {:?}", checker, e);
                        return None;
                    }
                };

                match IpAddr::from_str(string.trim()) {
                    Ok(ip) => Some(ip),
                    Err(e) => {
                        info!("checker {} returned {:?}", checker, e);
                        None
                    }
                }
            }
        })
        .collect();

    // We pick the most common API response.
    let mut guesses = HashMap::new();
    let mut max = 0;
    let mut arg_max = None;

    while let Some(check) = checks.next().await {
        if let Some(ip_address) = check {
            let entry = guesses.entry(ip_address).or_insert(0);
            *entry += 1;
            if *entry > max {
                max = *entry;
                arg_max = Some(ip_address);
            }
        }
    }

    if let Some(ip) = arg_max {
        info!(
            "got public IP {ip} (confirmed by {max}/{} 3rd parties)",
            checkers.len()
        );
    } else {
        warn!("failed to get public IP");
    }

    arg_max
}
