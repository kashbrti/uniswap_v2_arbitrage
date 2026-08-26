//! Create a connection to Alchemy for collecting Sync logs.

use alloy::primitives::Address;
use alloy::providers::{Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolEvent;
use futures::StreamExt;
use serde::Deserialize;
use std::env;
use std::fs;
use anyhow::{Context, Result};
use crate::modules::types::{Pool, Pools, Token, Sync}; 






pub(crate) fn get_pools() -> Result<Vec<Pool>> {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("pools.toml");
    let content = fs::read_to_string(&path)
        .with_context(|| format!("failed to open {}", path.display()))?;
    let file: Pools = toml::from_str(&content).context("failed to parse pools.toml")?;
    Ok(file.pools)
}

/// Load `.env` and return `MAINNET_RPC_URL`.
pub(crate) fn get_url() -> Result<String> {
    dotenvy::dotenv().context("failed to load .env file")?;
    let url = env::var("MAINNET_RPC_URL").context("MAINNET_RPC_URL not set")?;
    println!("loaded MAINNET_RPC_URL ({} chars)", url.len());
    Ok(url)
}

pub(crate) async fn connect(url: String, pools: &[Pool]) -> Result<()> {
    let ws = WsConnect::new(url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;
    println!("connection to the RPC secured");

    let addresses: Vec<Address> = pools.iter().map(|p| p.address).collect();
    let filter = Filter::new()
        .address(addresses)
        .event_signature(Sync::SIGNATURE_HASH);


    let sub = provider.subscribe_logs(&filter).await?;
    let mut stream = sub.into_stream();

    println!("listening for live Uniswap V2 Sync events");

    while let Some(log) = stream.next().await {
        let pool_address = log.address();
        println!("Sync log from {pool_address}");

        if let Ok(decoded) = log.log_decode::<Sync>() {
            let Sync { reserve0, reserve1 } = decoded.inner.data;
            println!("Pool: {pool_address:?}, Reserve0: {reserve0:?}, Reserve1: {reserve1:?}");
        }
    }

    Ok(())
}


async fn sanity_check_last_sync(url: String, pools :Vec<Pool>) -> Result<()>{

    let ws = WsConnect::new(url);
    let provider = ProviderBuilder::new().connect_ws(ws).await?;
    println!("connection to the RPC secured");

    let addresses: Vec<Address> = pools.iter().map(|p| p.address).collect();
    

    let tip = provider.get_block_number().await?; 
    let from = tip.saturating_sub(6); 

    let filter = Filter::new()
        .address(addresses)
        .event_signature(Sync::SIGNATURE_HASH)
        .from_block(from).to_block(tip);

    // Get the logs for the last 6 blocks 
    let recent = provider.get_logs(&filter).await?; 

    println!("sanity: {} Sync log(s) in blocks {from}..={tip}", recent.len());
    if recent.is_empty() {
        println!("warning: no Sync in lookback (filter may still be fine)");
    } else if let Ok(decoded) = recent[0].log_decode::<Sync>() {
        println!("sample: {} -> {:?}", recent[0].address(), decoded.inner.data);
    }




    Ok(())
    

}



#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn test_connect_loads_mainnet_rpc_url_from_env() {
        let url = get_url().unwrap();

        assert!(!url.is_empty(), "MAINNET_RPC_URL must not be empty");
        assert!(
            url.starts_with("http://")
                || url.starts_with("https://")
                || url.starts_with("ws://")
                || url.starts_with("wss://"),
            "MAINNET_RPC_URL must be an http(s)/ws(s) endpoint, got: {url}"
        );
    }


    #[test]
    fn test_read_pools_from_toml() {
        let pools = get_pools().expect("pools.toml should load");
        // for pool in &pools{
        //     println!("the pools are: {:?}", &pool.clone()); 
        // }
        assert_eq!(pools.len(), 3);
    }

    #[tokio::test]
    async fn test_connection() -> Result<()> {
        let url = get_url()?;
        let pools = get_pools()?;

        // `connect` runs until the subscription ends; bound only the test.
        match tokio::time::timeout(Duration::from_secs(45), connect(url, &pools)).await {
            Ok(result) => result,
            Err(_) => {
                // Still listening after 45s → WS + subscription are healthy.
                Ok(())
            }
        }
    }

    #[tokio::test]
    async fn test_sanity_check() -> Result<()>{
        let url = get_url()?; 
        let pools = get_pools()?;
        sanity_check_last_sync(url, pools).await?; 
        Ok(())
    }
}
