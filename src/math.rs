use basis_points::BasisPoints;
use brine_fp::UnsignedNumeric;
use quasar_lang::prelude::ProgramError;
use solana_math::{SafeConvert, SafeMath};

use crate::errors::FloatingOddsExchangeError;

/// `supply_buy` / `supply_other` are mint supplies of the purchased and
/// opposite outcomes.
///
/// Price: `P = supply_buy / (supply_buy + supply_other)`.
///
/// Cost for minting `amount` of the buy side:
/// ```text
/// cost = ceil(PRECISION * ∫_0^amount P(supply_buy + t) dt)
///      = ceil(PRECISION * (
///            amount
///          - supply_other
///            * ln((supply_buy + amount + supply_sell)
///                 / (supply_buy + supply_other))
///        ))
/// ```
/// When `supply_other == 0`, price is identically 1 and `cost = amount * PRECISION`.
/// Returns the `PRECISION`-scaled base cost, or an error if `amount == 0` or
/// both supplies are zero.
#[inline(never)]
pub fn quote_exact_output(
    supply_buy: u64,
    supply_other: u64,
    amount: u64,
    precision: u64,
) -> Result<u64, ProgramError> {
    if amount == 0 {
        return Err(FloatingOddsExchangeError::InvalidAmount.into());
    }

    let amount_u = amount as u128;
    let precision_u = precision as u128;

    if supply_other == 0 {
        let cost = amount_u.safe_mul(precision_u)?;
        return Ok(cost.safe_to_u64()?);
    }

    let supply_buy_u = supply_buy as u128;
    let supply_other_u = supply_other as u128;
    let total_before = supply_buy_u.safe_add(supply_other_u)?;

    if total_before == 0 {
        return Err(FloatingOddsExchangeError::InsufficientLiquidity.into());
    }

    let total_after = total_before.safe_add(amount_u)?;

    let ratio = UnsignedNumeric::new(total_after).safe_div(UnsignedNumeric::new(total_before))?;
    let ln = ratio
        .log()
        .ok_or(FloatingOddsExchangeError::ArithmeticOverflow)?;
    if ln.is_negative {
        return Err(FloatingOddsExchangeError::ArithmeticOverflow.into());
    }

    let ln_term = UnsignedNumeric::new(supply_other_u).safe_mul(ln.value)?;
    let integral = UnsignedNumeric::new(amount_u).safe_sub(ln_term)?;
    let cost_fp = integral.safe_mul(UnsignedNumeric::new(precision_u))?;
    let cost = cost_fp
        .ceiling()
        .ok_or(FloatingOddsExchangeError::ArithmeticOverflow)?
        .to_imprecise()
        .ok_or(FloatingOddsExchangeError::ArithmeticOverflow)?;

    Ok(cost.safe_to_u64()?)
}

/// Exact-input counterpart of [`quote_exact_output`].
///
/// `supply_buy` / `supply_other` are mint supplies of the purchased and
/// opposite outcomes. Given a `PRECISION`-scaled budget
/// `estimated_cost`, binary-searches the largest whole `amount_out` whose
/// integral cost (via [`quote_exact_output`]) fits the budget.
///
/// Returns `(amount_out, actual_cost)` where `actual_cost <= estimated_cost`.
/// Errors if `estimated_cost == 0` or no positive amount fits the budget.
#[inline(never)]
pub fn quote_exact_input(
    supply_buy: u64,
    supply_other: u64,
    estimated_cost: u64,
    precision: u64,
) -> Result<(u64, u64), ProgramError> {
    if estimated_cost == 0 {
        return Err(FloatingOddsExchangeError::InvalidAmount.into());
    }

    let precision_u = precision as u128;
    // Each unit costs at most `PRECISION` base units (price ≤ 1).
    let mut high = (estimated_cost as u128 / precision_u.max(1))
        .safe_to_u64()?
        .saturating_add(1);
    // Low-price markets can yield more than `cost/PRECISION` units.
    high = high
        .saturating_add(supply_other)
        .saturating_add(supply_buy)
        .max(1);

    let mut low = 1u64;
    let mut best_amount = 0u64;
    let mut best_cost = 0u64;

    while low <= high {
        let mid = low + ((high - low) >> 1);
        match quote_exact_output(supply_buy, supply_other, mid, precision) {
            Ok(cost) if cost <= estimated_cost => {
                best_amount = mid;
                best_cost = cost;
                low = mid.saturating_add(1);
            }
            Ok(_) => {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            }
            Err(_) => {
                if mid == 0 {
                    break;
                }
                high = mid - 1;
            }
        }
    }

    if best_amount == 0 {
        return Err(FloatingOddsExchangeError::InsufficientLiquidity.into());
    }

    Ok((best_amount, best_cost))
}

/// `ceil(pot_amount * fee_bps / 10_000)`.
#[inline(always)]
pub fn fee_from_pot(pot_amount: u64, fee_bps: u16) -> Result<u64, ProgramError> {
    let denom = BasisPoints::MAX as u128;
    let numer = (pot_amount as u128).safe_mul(u128::from(BasisPoints::new(fee_bps)?))?;
    // `ceil(n / d) = (n + d - 1) / d` for unsigned integers.
    let fee = numer.safe_add(denom.safe_sub(1)?)?.safe_div(denom)?;

    Ok(fee.safe_to_u64()?)
}

/// `floor(pot_amount * mint_amount / mint_supply)`.
#[inline(always)]
pub fn claims_from_pot(
    mint_amount: u64,
    mint_supply: u64,
    pot_amount: u64,
) -> Result<u64, ProgramError> {
    if mint_amount == 0 {
        return Err(FloatingOddsExchangeError::InvalidAmount.into());
    }
    if mint_supply == 0 {
        return Err(FloatingOddsExchangeError::InsufficientLiquidity.into());
    }

    let winnings = (pot_amount as u128)
        .safe_mul(mint_amount as u128)?
        .safe_div(mint_supply as u128)?;

    Ok(winnings.safe_to_u64()?)
}
