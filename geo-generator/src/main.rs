use std::collections::HashMap;
use std::net::IpAddr;
use std::time::Duration;

use serde::Deserialize;
use solana_client::rpc_client::RpcClient;

const SOLANA_RPC: &str = "https://api.mainnet-beta.solana.com";
const BATCH_SIZE: usize = 50;

#[derive(Deserialize, Debug)]
struct IpInfoLite {
    continent_code: Option<String>,
    country_code: Option<String>,
}

/// Map IP geolocation to a coarse geographic label.
/// We use continent-level granularity, with "Middle East" split out from Asia.
fn to_geo_label(info: &IpInfoLite) -> &'static str {
    let cc = info.country_code.as_deref().unwrap_or("");
    let continent = info.continent_code.as_deref().unwrap_or("");

    // Split Middle East + South Asia out from the generic "Asia" bucket
    match cc {
        "AE" | "SA" | "QA" | "BH" | "KW" | "OM" | "IR" | "IQ" | "IL" | "JO" | "LB" | "TR"
        | "PK" | "IN" | "BD" | "LK" | "EG" => return "Middle East",
        _ => {}
    }

    match continent {
        "EU" => "Europe",
        "NA" => "North America",
        "SA" => "South America",
        "AF" => "Africa",
        "AS" => "Asia",
        "OC" => "Oceania",
        _ => "UNKNOWN",
    }
}

async fn lookup_ip(
    client: &reqwest::Client,
    ip: IpAddr,
    token: &str,
) -> Option<(IpAddr, &'static str)> {
    let url = format!("https://api.ipinfo.io/lite/{}?token={}", ip, token);
    match client
        .get(&url)
        .send()
        .await
        .and_then(|r| r.error_for_status())
    {
        Ok(resp) => match resp.json::<IpInfoLite>().await {
            Ok(info) => Some((ip, to_geo_label(&info))),
            Err(e) => {
                eprintln!("  parse error for {}: {}", ip, e);
                None
            }
        },
        Err(e) => {
            eprintln!("  failed to look up {}: {}", ip, e);
            None
        }
    }
}

#[tokio::main]
async fn main() {
    let token = std::env::var("IPINFO_TOKEN").expect("set IPINFO_TOKEN env var");

    let rpc = RpcClient::new(SOLANA_RPC.to_string());

    eprintln!("fetching cluster nodes...");
    let nodes = rpc
        .get_cluster_nodes()
        .expect("failed to get cluster nodes");
    eprintln!("got {} nodes", nodes.len());

    let mut pubkey_ips: Vec<(String, IpAddr)> = Vec::new();
    for node in &nodes {
        if let Some(ip) = node
            .gossip
            .map(|a| a.ip())
            .filter(|ip| !ip.is_loopback() && !ip.is_unspecified())
        {
            pubkey_ips.push((node.pubkey.clone(), ip));
        }
    }
    eprintln!("{} nodes with usable IPs", pubkey_ips.len());

    let unique_ips: Vec<IpAddr> = {
        let mut seen = std::collections::HashSet::new();
        pubkey_ips
            .iter()
            .filter_map(|(_, ip)| seen.insert(*ip).then_some(*ip))
            .collect()
    };
    eprintln!("{} unique IPs to look up", unique_ips.len());

    let client = reqwest::Client::builder()
        .timeout(Duration::from_secs(10))
        .build()
        .expect("failed to build http client");

    let mut ip_to_geo: HashMap<IpAddr, &'static str> = HashMap::new();
    for (batch_idx, chunk) in unique_ips.chunks(BATCH_SIZE).enumerate() {
        let futures: Vec<_> = chunk
            .iter()
            .map(|ip| lookup_ip(&client, *ip, &token))
            .collect();
        let results = futures::future::join_all(futures).await;

        for result in results.into_iter().flatten() {
            ip_to_geo.insert(result.0, result.1);
        }
        eprintln!(
            "  batch {}: {}/{} done",
            batch_idx + 1,
            ip_to_geo.len(),
            unique_ips.len()
        );
    }

    let mut geo_map: HashMap<&str, &str> = HashMap::new();
    for (pubkey, ip) in &pubkey_ips {
        if let Some(geo) = ip_to_geo.get(ip) {
            geo_map.insert(pubkey, geo);
        }
    }

    eprintln!("mapped {} validators", geo_map.len());

    // print region distribution
    let mut counts: HashMap<&str, usize> = HashMap::new();
    for geo in geo_map.values() {
        *counts.entry(geo).or_default() += 1;
    }
    let mut counts: Vec<_> = counts.into_iter().collect();
    counts.sort_by(|a, b| b.1.cmp(&a.1));
    for (geo, count) in &counts {
        eprintln!("  {}: {}", geo, count);
    }

    let json = serde_json::to_string(&geo_map).expect("failed to serialize");
    println!("{}", json);
}
