//! Create a connection to Alchemy for collecting Sync logs.

use alloy::primitives::Address;
use std::collections::HashMap;
use alloy::providers::{DynProvider, Provider, ProviderBuilder, WsConnect};
use alloy::rpc::types::Filter;
use alloy::sol;
use alloy::sol_types::SolEvent;
use futures::StreamExt;
use serde::Deserialize;
use std::env;
use std::fs;
use anyhow::{Context, Result};
use artemis_light::types::{Collector, CollectorStream}; 
use async_trait::async_trait; 
use crate::modules::types::{Sync, Pools, Pool, Token, Event}; 
use crate::modules::connect::{get_pools, get_url}; 
use std::sync::Arc; 


pub(crate) struct V2SyncCollector{
    provider: DynProvider, 
    pool_addresses: Vec<Address>, 
    pool_map: HashMap<Address, Pool>,
}


impl V2SyncCollector{
    pub async fn new(url: String, pools: &[Pool]) -> Result<Self>{
        let ws = WsConnect::new(url); 
        let provider = ProviderBuilder::new().connect_ws(ws).await?.erased(); 
        
        let mut pool_map = HashMap::<Address, Pool>::new();
        let mut pool_addresses = Vec::<Address>::new(); 
        for pool in pools {
            let address = pool.address; 
            pool_addresses.push(address.clone());
       
            pool_map.insert(address, pool.clone());
        }

        Ok(Self {
            provider,
            pool_addresses,
            pool_map,
        })
    }
}


#[async_trait]
impl Collector<Event> for V2SyncCollector {
    async fn subscribe(&self) -> Result<CollectorStream<'_, Event>>{
        let filter = Filter::new()
            .address(self.pool_addresses.clone())
            .event_signature(Sync::SIGNATURE_HASH);
        let sub = self.provider.subscribe_logs(&filter).await?;
        let pool_map = self.pool_map.clone(); 

        let stream = sub.into_stream().filter_map(move |log| {
            let pool = pool_map.get(&log.address()).cloned();
            async move {
                let pool = pool?;
                let decoded = log.log_decode::<Sync>().ok()?;
                Some(Event::PoolSync {
                    pool,
                    reserve0: decoded.inner.data.reserve0,
                    reserve1: decoded.inner.data.reserve1,
                })
            }
        });

        Ok(Box::pin(stream))
    }
    
}

mod test {
    use tracing::event;

use super::*; 
use std::time::Duration;

    #[tokio::test]
    async fn test_uniswap_sync_collector() -> Result<()>{
        let url = get_url().context("failed to get the url, make sure the URL is set up")?; 
        let pools = get_pools().context("failed to get the pools, check toml")?;
        let collector = V2SyncCollector::new(url, &pools).await?; 

        let mut stream = collector.subscribe().await?; 

        match tokio::time::timeout(Duration::from_secs(180), async {
            while let Some(event) = stream.next().await {
                match event {
                    Event::PoolSync { pool, reserve0, reserve1 } => {
                        println!("a swap landed on: {}, pool: {}, address:{}, from:{}, amount: {}, to:{}, amount: {}",pool.venue, pool.id, pool.address, pool.token0.symbol, reserve0, pool.token1.symbol, reserve1);
                    }
                }
            }
        }).await {
            Ok(()) => Ok(()),
            Err(_) => Ok(()),
        }
    }
}