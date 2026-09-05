#![cfg_attr(not(test), no_std)]
#![allow(dead_code)]

use quasar_lang::prelude::*;

mod constants;
mod errors;
mod events;
mod instructions;
mod state;
use constants::{NAME_MAX_LEN, SYMBOL_MAX_LEN, URI_MAX_LEN};
use instructions::*;

declare_id!("JDLx5QurZhzV5bVVwEm77b5bQdG74m4oALwqzYbNmBs");

#[program]
mod floating_odds_exchange {
    use super::*;

    #[instruction(discriminator = 0)]
    pub fn initialize(
        ctx: Ctx<Initialize>,
        seed: u64,
        fee_bps: u16,
        name: String<NAME_MAX_LEN>,
        symbol: String<SYMBOL_MAX_LEN>,
        uri: String<URI_MAX_LEN>,
    ) -> Result<(), ProgramError> {
        ctx.accounts
            .handler(&ctx.bumps, seed, fee_bps, name, symbol, uri)
    }

    #[instruction(discriminator = 1)]
    pub fn launch(ctx: Ctx<Launch>, amount_y: u64, amount_n: u64) -> Result<(), ProgramError> {
        ctx.accounts.handler(amount_y, amount_n)
    }

    #[instruction(discriminator = 2)]
    pub fn buy(
        ctx: Ctx<Buy>,
        in_amount: u64,
        out_amount: u64,
        exact_in: bool,
        amount_with_slippage: u64,
    ) -> Result<(), ProgramError> {
        ctx.accounts
            .handler(in_amount, out_amount, exact_in, amount_with_slippage)
    }

    #[instruction(discriminator = 3)]
    pub fn claim(ctx: Ctx<Claim>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }

    #[instruction(discriminator = 4)]
    pub fn resolve(ctx: Ctx<Resolve>, outcome: u8) -> Result<(), ProgramError> {
        ctx.accounts.handler(outcome)
    }

    #[instruction(discriminator = 5)]
    pub fn pause(ctx: Ctx<Pause>) -> Result<(), ProgramError> {
        ctx.accounts.handler()
    }
}

#[cfg(all(test, not(feature = "idl-build")))]
mod tests;
