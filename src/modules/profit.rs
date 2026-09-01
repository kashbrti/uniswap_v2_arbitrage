use alloy::primitives::U256; 
use crate::modules::math::get_amount_out; 

pub fn calculate_net_profit(
    amount_in: U256,
    r1_a: U256, 
    r1_b: U256,
    r2_a: U256, 
    r2_b: U256, 
    estimated_gas_used: U256, 
    base_fee_per_gas: U256, 
) -> Option<U256>{
    let amount_b_out = get_amount_out(amount_in, r1_a, r1_b); 
    if amount_in.is_zero(){
        return None; 
    }

    let amount_a_final = get_amount_out(amount_b_out, r2_b, r2_a); 

    if amount_a_final <= amount_in {
        return None; 
    }

    let gross_profit = amount_a_final - amount_in; 
    let total_gas_cost = estimated_gas_used * base_fee_per_gas; 

    if gross_profit > total_gas_cost {
        return Some(gross_profit - total_gas_cost); 
    }

    return None; 
}

