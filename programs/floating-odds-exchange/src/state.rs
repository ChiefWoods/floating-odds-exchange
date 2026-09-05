use quasar_lang::prelude::*;

use crate::errors::FloatingOddsExchangeError;

#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outcome {
    Yes = 0,
    No = 1,
    Undecided = 2,
    Refunded = 3,
}

impl From<Outcome> for u8 {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Yes => 0,
            Outcome::No => 1,
            Outcome::Undecided => 2,
            Outcome::Refunded => 3,
        }
    }
}

impl TryFrom<u8> for Outcome {
    type Error = ProgramError;

    #[inline(always)]
    fn try_from(value: u8) -> Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Yes),
            1 => Ok(Self::No),
            2 => Ok(Self::Undecided),
            3 => Ok(Self::Refunded),
            _ => Err(FloatingOddsExchangeError::InvalidOutcome.into()),
        }
    }
}

#[derive(Seeds)]
#[seeds(b"mint_y", market: Address)]
pub struct MintY;

#[derive(Seeds)]
#[seeds(b"mint_n", market: Address)]
pub struct MintN;

#[account(discriminator = 1, set_inner)]
#[seeds(b"market", authority: Address, seed: u64)]
pub struct Market {
    pub seed: u64,
    pub authority: Address,
    pub payout: Address,
    pub pot_mint: Address,
    pub precision: u64,
    pub outcome: u8,
    pub fee_bps: u16,
    pub mint_y_bump: u8,
    pub mint_n_bump: u8,
    pub bump: u8,
    pub is_launched: bool,
    pub is_paused: bool,
}
