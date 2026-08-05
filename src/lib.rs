#![cfg_attr(not(test), no_std)]

use quasar_lang::prelude::*;

mod errors;
mod instructions;
mod state;
use instructions::*;

declare_id!("5pnZaYWVefsLgenmuseF8apDUZjjV1teJ6k1hNSYYANB");

#[program]
mod floating_odds_exchange {
    use super::*;

    #[instruction]
    pub fn initialize(ctx: Ctx<Initialize>) -> Result<(), ProgramError> {
        ctx.accounts.initialize()
    }
}

#[cfg(test)]
mod tests;
