use alloy::primitives::{U256, U512};

const FEE_NUMERATOR: u64 = 997;
const FEE_DENOMINATOR: u64 = 1000;

pub(crate) fn get_amount_out(amount_in: U256, reserve_in: U256, reserve_out: U256) -> U256 {
    if amount_in.is_zero() || reserve_in.is_zero() || reserve_out.is_zero() {
        return U256::ZERO;
    }

    let amount_in_with_fee: U256 = amount_in * U256::from(FEE_NUMERATOR); 
    let numerator = amount_in_with_fee * reserve_out; 
    let denominator = (reserve_in * U256::from(FEE_DENOMINATOR)) + amount_in_with_fee; 

    numerator / denominator 
}

pub(crate) fn get_optimial_arbitrage_amount(
    r1_a: U256,
    r1_b: U256,
    r2_a: U256,
    r2_b: U256,
) -> Option<U256> {
    let any_zero = r1_a.is_zero() || r1_b.is_zero() || r2_a.is_zero() || r2_b.is_zero(); 
    if any_zero{
        return None;
    }

    /// By taking the derivative of the nested uniswap formula we have that the value is maximized when 
    /// $$\delta_A = \frac{\sqrt{CK} - K}{M}$$
    /// Where: 
    /// C = \gamma^2*r1b*r2a 
    /// K = r1a*r2b
    /// M = \gamma * (r2b + \gamma r1b)
    /// 
    let fee_numerator = U256::from(FEE_NUMERATOR);
    let fee_numerator_sqr = U256::from(FEE_NUMERATOR * FEE_NUMERATOR);
    let fee_denominator = U256::from(FEE_DENOMINATOR) ;
    let fee_denominator_sqr = U256::from(FEE_DENOMINATOR * FEE_DENOMINATOR);
    let r1br2a = r1_b.checked_mul(r2_a)?; 
    let raw_c = r1br2a * fee_numerator_sqr;  // c * denumerator^2 
    let k = r1_a * r2_b; 
    let raw_m = fee_numerator.checked_mul(
        r2_b.checked_mul(fee_denominator)?
            .checked_add(fee_numerator.checked_mul(r1_b)?)?,
    )?; // m * fee_denominator^2

    let sqrt_ck = U256::from((U512::from(raw_c) * U512::from(k)).root(2));

    if raw_c > k.checked_mul(fee_denominator_sqr)? {
        let num = fee_denominator
            .checked_mul(sqrt_ck)?
            .checked_sub(k.checked_mul(fee_denominator_sqr)?)?;
        return Some(num.checked_div(raw_m)?);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn u(x: u64) -> U256 {
        U256::from(x)
    }

    /// A -> B on pool 1, then B -> A on pool 2.
    fn a_round_trip(r1_a: U256, r1_b: U256, r2_a: U256, r2_b: U256, delta_a: U256) -> U256 {
        let b_out = get_amount_out(delta_a, r1_a, r1_b);
        get_amount_out(b_out, r2_b, r2_a)
    }

    #[test]
    fn returns_none_for_zero_reserves() {
        assert!(get_optimial_arbitrage_amount(u(0), u(1_000), u(1_000), u(1_000)).is_none());
    }

    #[test]
    fn returns_none_when_pools_are_balanced() {
        assert!(get_optimial_arbitrage_amount(u(1_000), u(1_000), u(1_000), u(1_000)).is_none());
    }

    #[test]
    fn returns_profitable_size_when_pool1_overprices_a() {
        // Pool 1: A is rich in B (sell A here). Pool 2: cheaper A.
        let r1_a = u(1_000);
        let r1_b = u(1_200);
        let r2_a = u(1_000);
        let r2_b = u(1_000);

        let delta = get_optimial_arbitrage_amount(r1_a, r1_b, r2_a, r2_b)
            .expect("mispriced pools should yield an arb size");

        assert!(delta > U256::ZERO);

        let a_back = a_round_trip(r1_a, r1_b, r2_a, r2_b, delta);
        assert!(a_back > delta, "round trip should return more A than input");

        // Near-optimal: beat slightly smaller and slightly larger sizes.
        if delta > U256::ONE {
            let profit_at_delta = a_back - delta;
            let profit_below = a_round_trip(r1_a, r1_b, r2_a, r2_b, delta - U256::ONE) - (delta - U256::ONE);
            let profit_above = a_round_trip(r1_a, r1_b, r2_a, r2_b, delta + U256::ONE) - (delta + U256::ONE);
            assert!(profit_at_delta >= profit_below);
            assert!(profit_at_delta >= profit_above);
        }
    }
}
