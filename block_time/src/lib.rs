use chrono::Utc;
use serde::Serialize;
use zela_std::{CustomProcedure, JsonValue, rpc_client::RpcClient, zela_custom_procedure};

// Define an empty struct to serve as a binding to rockrpc_custom_procedure trait.
pub struct BlockTime;

// Define output data of your method
#[derive(Serialize)]
pub struct TimeCheck {
    pub block_time: i64,
    pub block_hash: String,
    pub system_time: i64,
    pub time_elapsed: i64,
}

impl CustomProcedure for BlockTime {
    // We do not need any params for this procedure
    type Params = ();
    type SuccessData = TimeCheck;
    type ErrorData = JsonValue;

    // Run method is the entry point of every custom procedure.
    // It will be called once for each incoming request.
    async fn run(
        _: Self::Params,
    ) -> Result<Self::SuccessData, zela_std::RpcError<Self::ErrorData>> {
        let start = Utc::now();

        let (block_time, block_hash) = {
            // Initialize RockRPC Solana RPC proxy client.
            // Its interface is the same as in the solana_client crate.
            let client = RpcClient::new();

            let slot = client.get_slot().await?;

            (
                client.get_block_time(slot).await?,
                client
                    .get_latest_blockhash()
                    .await
                    .map(|hash| hash.to_string())?,
            )
        };

        let end = Utc::now();

        let time_elapsed = (end - start).num_microseconds().unwrap_or_default();

        // Assemble response struct.
        // It will be serialized into the JSON response using serde_json.
        let response = TimeCheck {
            block_time,
            block_hash,
            time_elapsed,
            system_time: start.timestamp_millis(),
        };

        Ok(response)
    }
}

// This is an essential macro-call that enables us to run a procedure
zela_custom_procedure!(BlockTime);
