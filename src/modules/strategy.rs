use crate::modules::gas::AtomicStateView;
use crate::modules::profit::calculate_net_profit; 
use alloy::primitives::Address;
use futures::stream;
use std::collections::HashMap;
use std::thread::current;
use alloy::providers::{Provider};
use alloy::sol;
use anyhow::{Context, Result};
use artemis_light::types::{ActionStream, Strategy}; 
use async_trait::async_trait; 
use crate::modules::math::get_optimial_arbitrage_amount;
use crate::modules::types::{Event, Pool, PoolState, Action, Token}; 
use alloy::primitives::aliases::U256;



sol! {
    #[sol(rpc)]
    interface IUniswapV2Pair {
        function getReserves() external view returns (uint112 reserve0, uint112 reserve1, uint32 blockTimestampLast);
    }
}
pub(crate) struct ArbitrageStrategy<P>{
    state_view: AtomicStateView, 
    provider: P, 
    initial_pools: Vec<Pool>, 
    pools: HashMap<Address, PoolState>, 
    estimated_gas_used: U256,

}

impl <P: Provider + Clone> ArbitrageStrategy<P> {
    pub(crate) async fn new(provider: P, pools:Vec<Pool>, state_view: AtomicStateView)-> Self{
        Self {
            state_view: state_view, 
            provider: provider, 
            initial_pools: pools, 
            pools: HashMap::<Address, PoolState>::new(),
            estimated_gas_used: U256::from(140_000),
        }
    }


    pub(crate) fn evaluation_opportunities(&self, updated_address: Address, base_fee_per_gas: U256) -> Vec<Action> {
        let mut actions: Vec<Action> = Vec::new(); 
        // Get the state that was changed from the latest update 
        let current_state = match self.pools.get(&updated_address){
            Some(state) => state, 
            None => return actions, 
        }; 

        if current_state.reserve0.is_zero() || current_state.reserve1.is_zero() {
            return actions;
        }

        for (other_address, other_state) in &self.pools {
            if other_address == &updated_address {
                continue; 
            } 
            if other_state.reserve0.is_zero() || other_state.reserve1.is_zero(){
                continue; 
            } 

            let (r1_a, r1_b, r2_a, r2_b) = if current_state.pool.token0 == other_state.pool.token0
                && current_state.pool.token1 == other_state.pool.token1
            {
                (
                    current_state.reserve0,
                    current_state.reserve1,
                    other_state.reserve0,
                    other_state.reserve1,
                )
            } else if current_state.pool.token0 == other_state.pool.token1
                && current_state.pool.token1 == other_state.pool.token0
            {
                (
                    current_state.reserve0,
                    current_state.reserve1,
                    other_state.reserve1,
                    other_state.reserve0,
                )
            } else {
                continue;
            };

            if let Some(amount_in) = get_optimial_arbitrage_amount(r1_a, r1_b, r2_a, r2_b) {
                // if !amount_in.is_zero() {
                //     actions.push(Action::SubmitArbBundle {
                //         pool_a: current_state.pool.clone(),
                //         pool_b: other_state.pool.clone(),
                //         amount_in,
                //         expected_profit: U256::ZERO,
                //     });
                // }
                if let Some(net_profit) = calculate_net_profit(amount_in, r1_a, r1_b, r2_a, r2_b, self.estimated_gas_used, base_fee_per_gas){
                    actions.push(Action::SubmitArbBundle {
                                pool_a: current_state.pool.clone(),
                                pool_b: other_state.pool.clone(),
                                amount_in,
                                expected_profit: net_profit,
                            });

                }
            }
            if let Some(amount_in) = get_optimial_arbitrage_amount(r2_a, r2_b, r1_a, r1_b) {
                if let Some(net_profit) = calculate_net_profit(
                    amount_in,
                    r2_a,
                    r2_b,
                    r1_a,
                    r1_b,
                    self.estimated_gas_used,
                    base_fee_per_gas,
                ) {
                    actions.push(Action::SubmitArbBundle {
                        pool_a: other_state.pool.clone(),
                        pool_b: current_state.pool.clone(),
                        amount_in,
                        expected_profit: net_profit,
                    });
                }
            }
        }

        return actions;
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
                let base_fee_per_gas = self.state_view.get_base_fee();
                let actions = self.evaluation_opportunities(pool.address, base_fee_per_gas);
                Ok(Box::pin(stream::iter(actions)))
            }
        }

    }


    
}