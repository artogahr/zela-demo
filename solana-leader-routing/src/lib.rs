use std::collections::HashMap;

use serde::Serialize;

#[cfg(target_arch = "wasm32")]
use zela_std::rpc_client::RpcClient;
#[cfg(not(target_arch = "wasm32"))]
use solana_client::nonblocking::rpc_client::RpcClient;

const GEO_MAP_JSON: &str = include_str!("geo_map.json");

pub struct SolanaLeaderRouter;

#[derive(Serialize, Debug)]
pub struct Output {
    pub slot: u64,
    pub leader: String,
    pub leader_geo: String,
    pub closest_region: String,
}

fn geo_to_region(geo: &str) -> &'static str {
    match geo {
        "Europe" | "Africa" => "Frankfurt",
        "North America" | "South America" => "NewYork",
        "Asia" | "Oceania" => "Tokyo",
        "Middle East" => "Dubai",
        _ => "Frankfurt",
    }
}

fn load_geo_map() -> HashMap<String, String> {
    serde_json::from_str(GEO_MAP_JSON).expect("embedded geo map is invalid")
}

impl SolanaLeaderRouter {
    pub async fn run(rpc: &RpcClient) -> Result<Output, String> {
        let slot = rpc.get_slot().await.map_err(|e| format!("get_slot: {e}"))?;

        let leaders = rpc
            .get_slot_leaders(slot, 1)
            .await
            .map_err(|e| format!("get_slot_leaders: {e}"))?;

        let leader = leaders
            .first()
            .ok_or("no leader returned for current slot")?;

        let leader_str = leader.to_string();
        let geo_map = load_geo_map();

        let (leader_geo, closest_region) = match geo_map.get(&leader_str) {
            Some(geo) => (geo.as_str(), geo_to_region(geo)),
            None => ("UNKNOWN", "Frankfurt"),
        };

        Ok(Output {
            slot,
            leader: leader_str,
            leader_geo: leader_geo.to_string(),
            closest_region: closest_region.to_string(),
        })
    }
}

#[cfg(target_arch = "wasm32")]
mod zela {
    use zela_std::{zela_custom_procedure, CustomProcedure, RpcError};

    use super::*;

    impl CustomProcedure for SolanaLeaderRouter {
        type Params = ();
        type ErrorData = ();
        type SuccessData = Output;

        async fn run(_: Self::Params) -> Result<Self::SuccessData, RpcError<Self::ErrorData>> {
            let rpc = RpcClient::new();
            SolanaLeaderRouter::run(&rpc).await.map_err(|e| RpcError {
                code: 1,
                message: e,
                data: None,
            })
        }

        const LOG_MAX_LEVEL: log::LevelFilter = log::LevelFilter::Debug;
    }

    zela_custom_procedure!(SolanaLeaderRouter);
}

#[cfg(test)]
mod tests {
    use super::*;
    use solana_sdk::commitment_config::CommitmentConfig;

    #[tokio::test]
    async fn test_leader_routing() {
        let rpc = RpcClient::new_with_commitment(
            "https://api.mainnet-beta.solana.com".to_string(),
            CommitmentConfig::confirmed(),
        );
        let output = SolanaLeaderRouter::run(&rpc).await.unwrap();
        println!("{:?}", output);

        assert!(output.slot > 0);
        assert!(!output.leader.is_empty());
        assert!(
            ["Frankfurt", "Dubai", "NewYork", "Tokyo"].contains(&output.closest_region.as_str())
        );
    }

    #[test]
    fn test_geo_map_loads() {
        let map = load_geo_map();
        assert!(map.len() > 1000, "geo map should have plenty of entries");
    }

    #[test]
    fn test_geo_to_region_mapping() {
        assert_eq!(geo_to_region("Europe"), "Frankfurt");
        assert_eq!(geo_to_region("North America"), "NewYork");
        assert_eq!(geo_to_region("Asia"), "Tokyo");
        assert_eq!(geo_to_region("Middle East"), "Dubai");
        assert_eq!(geo_to_region("Africa"), "Frankfurt");
        assert_eq!(geo_to_region("Oceania"), "Tokyo");
        assert_eq!(geo_to_region("South America"), "NewYork");
        assert_eq!(geo_to_region("something weird"), "Frankfurt");
    }
}
