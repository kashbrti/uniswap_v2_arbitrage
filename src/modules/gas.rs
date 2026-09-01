use alloy::primitives::U256;
use alloy::providers::Provider;
use anyhow::Result;
use futures::StreamExt;

use std::sync::{
    atomic::{AtomicU128, Ordering},
    Arc,
};



pub async fn get_next_block_base_gas<P: Provider>(provider: &P) -> Result<U256>{

    let fee_history = provider.get_fee_history(1, alloy::eips::BlockNumberOrTag::Latest, &[]).await?; 

    if let Some(next_base_fee) = fee_history.base_fee_per_gas.last() {
        Ok(U256::from(*next_base_fee))
    } else {
        let gas_price = provider.get_gas_price().await?;
        Ok(U256::from(gas_price))
    }

}






#[derive(Clone, Default)]
pub(crate) struct AtomicStateView {
    base_fee_wei: Arc<AtomicU128>,
    latest_block: Arc<AtomicU128>,
}

impl AtomicStateView {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set_base_fee(&self, base_fee: U256) {
        let wei = base_fee.to::<u128>();
        self.base_fee_wei.store(wei, Ordering::Relaxed);
    }

    pub fn get_base_fee(&self) -> U256 {
        U256::from(self.base_fee_wei.load(Ordering::Relaxed))
    }

    pub fn set_latest_block(&self, latest_block: u64) {
        self.latest_block
            .store(latest_block as u128, Ordering::Relaxed);
    }

    pub fn get_latest_block(&self) -> u64 {
        self.latest_block.load(Ordering::Relaxed) as u64
    }
}


pub fn spawn_block_watcher<P: Provider + Send + Sync + 'static>(provider: Arc<P>, state_view: AtomicStateView) -> Result<()> {
    tokio::spawn(async move {
        let sub = match provider.subscribe_blocks().await {
            Ok(sub) => sub,
            Err(e) => {
                eprintln!("could not subscribe for block headers: {e}");
                return;
            }
        };

        let mut stream = sub.into_stream();
        while let Some(header) = stream.next().await {
            if let Some(base_fee) = header.base_fee_per_gas {
                state_view.set_base_fee(U256::from(base_fee));
                state_view.set_latest_block(header.number);
            }
        }
    });

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modules::connect::get_url;
    use alloy::providers::{ProviderBuilder, WsConnect};
    use std::sync::Arc;
    use std::time::Duration;

    #[test]
    fn atomic_state_view_stores_and_loads() {
        let view = AtomicStateView::new();
        assert_eq!(view.get_base_fee(), U256::ZERO);
        assert_eq!(view.get_latest_block(), 0);

        view.set_base_fee(U256::from(25_000_000_000u64));
        view.set_latest_block(18_000_000);

        assert_eq!(view.get_base_fee(), U256::from(25_000_000_000u64));
        assert_eq!(view.get_latest_block(), 18_000_000);
    }

    #[tokio::test]
    async fn watcher_base_fee_matches_get_next_block_prediction() -> Result<()> {
        let url = get_url()?;
        let provider = Arc::new(ProviderBuilder::new().connect_ws(WsConnect::new(url)).await?);
        let state_view = AtomicStateView::new();
        spawn_block_watcher(provider.clone(), state_view.clone())?;

        tokio::time::timeout(Duration::from_secs(90), async {
            while state_view.get_latest_block() == 0 {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("block watcher did not receive an initial block within 90s"))?;

        let block_before = state_view.get_latest_block();
        let predicted_next_base_fee = get_next_block_base_gas(provider.as_ref()).await?;

        tokio::time::timeout(Duration::from_secs(90), async {
            while state_view.get_latest_block() <= block_before {
                tokio::time::sleep(Duration::from_millis(250)).await;
            }
        })
        .await
        .map_err(|_| anyhow::anyhow!("block watcher did not advance to the next block within 90s"))?;

        assert_eq!(
            state_view.get_base_fee(),
            predicted_next_base_fee,
            "watcher base fee at block {} should match get_next_block_base_gas prediction",
            state_view.get_latest_block(),
        );

        Ok(())
    }
}