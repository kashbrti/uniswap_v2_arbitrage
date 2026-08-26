use alloy::primitives::Address;
use std::collections::HashMap;
use alloy::providers::{Provider};
use alloy::sol;
use anyhow::{Context, Result};
use artemis_light::types::{ActionStream, Strategy}; 
use async_trait::async_trait; 
use crate::modules::types::{Event, Pool, PoolState, Action, Token}; 
use alloy::primitives::aliases::U256;



sol! {
    #[sol(rpc)]
    interface IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }
}
pub(crate) struct ArbitrageStrategy<P>{
    provider: P, 
    initial_pools: Vec<Pool>, 
    pools: HashMap<Address, PoolState>, 

}

impl <P: Provider + Clone> ArbitrageStrategy<P> {
    pub(crate) async fn new(provider: P, pools:Vec<Pool>)-> Self{
        Self {
            provider: provider, 
            initial_pools: pools, 
            pools: HashMap::<Address, PoolState>::new(),
        }
    }

    pub(crate) async fn get_arbitrage_action(&self) -> Action{
        let token0 = Token {
            address: Address::ZERO,
            symbol: "TOKEN0".to_string(),
            decimals: 18,
        };
        let token1 = Token {
            address: Address::repeat_byte(0x11),
            symbol: "TOKEN1".to_string(),
            decimals: 18,
        };
        let pool_a = Pool {
            id: 0,
            address: Address::repeat_byte(0xaa),
            venue: "uniswap_v2".to_string(),
            token0: token0.clone(),
            token1: token1.clone(),
        };
        let pool_b = Pool {
            id: 1,
            address: Address::repeat_byte(0xbb),
            venue: "uniswap_v2".to_string(),
            token0,
            token1,
        };
        Action::SubmitArbBundle {
            pool_a,
            pool_b,
            amount_in: U256::from(1_000_000_000_000_000_000u64),
            expected_profit: U256::from(1u64),
        }
    }
    
}


#[async_trait]
impl<P: Provider + Clone + Send + std::marker::Sync + 'static>  Strategy<Event, Action> for ArbitrageStrategy<P>{
    async fn sync_state(& mut self)-> Result<()>{
            println!("Artemis Engine: Syncing state"); 

            for pool in &self.initial_pools{
                let address = pool.address; // copy 
                // make the RPC call to get the reserves
                let pair = IUniswapV2Pair::new(address, &self.provider); 

                match pair.getReserves().call().await {
                    Ok(reserves) => {
                        let r0 = U256::from(reserves.reserve0);
                        let r1 = U256::from(reserves.reserve1); 
                        println!("Got reserves for pool {}. amounts: 1:{} 2:{}", pool.address,r0, r1); 
                        let pool_state = PoolState {pool: pool.clone(), reserve0: r0, reserve1: r1}; 
                        self.pools.insert(address, pool_state); 
                    }
                    Err(e) => {
                        println!("Could not get the reserves for pool with address: {}", address); 
                        self.pools.insert(address, PoolState {pool: pool.clone(), reserve0: U256::ZERO, reserve1: U256::ZERO}); 
                    }
            }
        }
        println!("Sucessfully bootstraped the reserve values"); 
        Ok(())
    }
    async fn process_event(&mut self, event: Event) -> Result<ActionStream<'_,Action>> {
        match event {
            Event::PoolSync { pool, reserve0, reserve1 } =>{
                let r0 = U256::from(reserve0); 
                let r1 = U256::from(reserve1); 

                if let Some(state) = self.pools.get_mut(&pool.address){
                    state.reserve0 = r0; 
                    state.reserve1 = r1; 
    
                }
                let action = self.get_arbitrage_action().await;
                Ok(Box::pin(futures::stream::iter(std::iter::once(action))))
            }
        }
    }


    
}