use alloy::primitives::aliases::{U112,U256};
use alloy::primitives::Address;
use alloy::sol;
use serde::Deserialize;

// a small module for the event types
#[derive(Deserialize, Debug, Clone)]
pub(crate) struct Pool {
    pub(crate) id: u32,
    pub(crate) address: Address,
    pub(crate) venue: String,
    pub(crate) token0: Token,
    pub(crate) token1: Token,
}

#[derive(Deserialize, Debug, Clone, PartialEq)]
pub(crate) struct Token {
    pub(crate) address: Address,
    pub(crate) symbol: String,
    pub(crate) decimals: u32,
}

#[derive(Deserialize, Debug)]
pub(crate) struct Pools {
    pub(crate) pools: Vec<Pool>,
}

sol! {
    #[derive(Debug)]
    event Sync(uint112 reserve0, uint112 reserve1);
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) enum Event {
    PoolSync {
        pool: Pool,
        reserve0: U112,
        reserve1: U112,
    },
}

#[derive(Debug, Clone)]
pub(crate) struct PoolState{
    pub(crate) pool: Pool, 
    pub(crate) reserve0: U256, 
    pub(crate) reserve1: U256,
}

#[derive(Debug, Clone)]
pub(crate) enum Action{
    SubmitArbBundle {
        pool_a: Pool, 
        pool_b: Pool, 
        amount_in: U256, 
        expected_profit:U256, 
    },
    None, 

}