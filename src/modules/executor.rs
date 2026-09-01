use alloy::{
    consensus::{SignableTransaction, TxLegacy}, 
    primitives::{Address, Bytes, U256}, 
    providers::Provider, 
    rpc::types::mev::EthSendBundle,
    signers::local::PrivateKeySigner,
    sol, 
};


use anyhow::Result;

use alloy::sol_types::SolCall;
use artemis_light::types::Executor; 
use async_trait::async_trait; 
use std::sync::atomic::{AtomicU64, Ordering}; 
use std::sync::Arc; 

use crate::modules::types::Action; 




sol! {
    interface IflashArbExecutor { 
        function executeFlashSwap(
            address poolA, 
            address poolB, 
            uint256 amountIn, 
            uint256 minProfit
        ) external; 
    }
}

pub struct FlashBotExecutor<P> {
    provider:P, 
    signer: PrivateKeySigner, 
    contract_address: Address, 
    flashbots_rpc_url: String, 
    nonce: Arc<AtomicU64>, 
}

impl<P: Provider> FlashBotExecutor<P>{
    pub async fn new(provider: P, signer: PrivateKeySigner, contract_address: Address, rpc_url: String)->Result<Self>{
        let initial_nonce = provider.get_transaction_count(signer.address()).await?; 
        Ok(Self {
            provider: provider,
            signer: signer, 
            contract_address: contract_address, 
            flashbots_rpc_url: rpc_url,
            nonce: Arc::new(AtomicU64::from(initial_nonce)), 
        })
    }

    pub(crate) async fn execute_bundle(&self, action: Action) -> Result<()> {
        let Action::SubmitArbBundle {
            pool_a,
            pool_b,
            amount_in,
            expected_profit,
        } = action
        else {
            return Ok(());
        };

        let calldata = IflashArbExecutor::executeFlashSwapCall {
            poolA: pool_a.address,
            poolB: pool_b.address,
            amountIn: amount_in,
            minProfit: expected_profit,
        }
        .abi_encode();

        let latest_block = self.provider.get_block_number().await?; 
        let target_block = latest_block + 1; 

        let nonce = self.nonce.fetch_add(1, Ordering::SeqCst); 

        let mut tx = TxLegacy{
            chain_id: Some(1), 
            nonce, 
            gas_price:0, 
            gas_limit: 200_000, 
            to: alloy::primitives::TxKind::Call(self.contract_address),
            value: U256::ZERO, 
            input: Bytes::from(calldata), 
        };

        Ok(())
    }
}