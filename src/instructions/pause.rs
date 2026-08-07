use quasar_lang::{prelude::*, sysvars::Sysvar};

use crate::{
    errors::FloatingOddsExchangeError,
    events::MarketPaused,
    state::{Market, Outcome},
    EventAuthority, FloatingOddsExchange,
};

#[derive(Accounts)]
pub struct Pause {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        constraints(market.is_launched.is_true()) @ FloatingOddsExchangeError::MarketNotLaunched,
        has_one(authority) @ FloatingOddsExchangeError::UnauthorizedAuthority
    )]
    pub market: Account<Market>,

    pub event_authority: EventAuthority,
    pub program: Program<FloatingOddsExchange>,
}

impl Pause {
    /// 1. Reject resolved markets.
    /// 2. Set market as paused.
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let Clock { slot, .. } = Clock::get()?;

        let market = &mut self.market;

        require!(
            market.outcome == u8::from(Outcome::Undecided),
            FloatingOddsExchangeError::MarketAlreadyResolved
        );

        market.is_paused.set(true);

        emit_cpi!(MarketPaused {
            market: *self.market.address(),
            slot: slot.into(),
        })?;

        Ok(())
    }
}
