use floating_odds_exchange_math::fee_from_pot;
use quasar_lang::{cpi::Seed, prelude::*, sysvars::Sysvar};
use quasar_spl::prelude::*;

use crate::{
    errors::FloatingOddsExchangeError,
    events::MarketResolved,
    state::{Market, Outcome},
    EventAuthority, FloatingOddsExchange,
};

#[derive(Accounts)]
pub struct Resolve {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        constraints(market.is_launched.is_true()) @ FloatingOddsExchangeError::MarketNotLaunched,
        constraints(market.outcome == u8::from(Outcome::Undecided)) @ FloatingOddsExchangeError::MarketAlreadyResolved,
        has_one(authority) @ FloatingOddsExchangeError::UnauthorizedAuthority,
        has_one(pot_mint) @ FloatingOddsExchangeError::InvalidMint,
    )]
    pub market: Account<Market>,

    pub pot_mint: InterfaceAccount<Mint>,

    #[account(mut)]
    pub mint_y: Account<Mint>,

    #[account(mut)]
    pub mint_n: Account<Mint>,

    #[account(
        mut,
        associated_token(
            mint = pot_mint,
            authority = market,
            token_program = token_program
        )
    )]
    pub pot: InterfaceAccount<Token>,

    #[account(
        mut,
        associated_token(
            mint = mint_y,
            authority = market,
            token_program = token_program,
        )
    )]
    pub vault_y: Account<Token>,

    #[account(
        mut,
        associated_token(
            mint = mint_n,
            authority = market,
            token_program = token_program,
        )
    )]
    pub vault_n: Account<Token>,

    // not validated, assumed to already exist
    #[account(mut)]
    pub payout: InterfaceAccount<Token>,

    pub system_program: Program<SystemProgram>,
    pub token_program: Interface<TokenInterface>,
    pub event_authority: EventAuthority,
    pub program: Program<FloatingOddsExchange>,
}

impl Resolve {
    /// 1. Parse and validate outcome.
    /// 2. Set market outcome.
    /// 3. Burn Y and N mints.
    /// 4. Close vaults.
    ///
    /// If outcome is not `Refunded`:
    /// 1. Calculate fee from pot.
    /// 2. Transfer fee to payout.
    #[inline(always)]
    pub fn handler(&mut self, outcome: u8) -> Result<(), ProgramError> {
        let Clock { slot, .. } = Clock::get()?;

        let outcome = Outcome::try_from(outcome)?;

        require!(
            outcome != Outcome::Undecided,
            FloatingOddsExchangeError::MarketIsAlreadyUndecided
        );

        let market = &mut self.market;

        market.outcome = outcome.into();

        let market_seed: u64 = market.seed.into();
        let market_seed_bytes = market_seed.to_le_bytes();
        let market_bump = [market.bump];

        let market_signer_seeds = [
            Seed::from(Market::SEED_PREFIX as &[u8]),
            Seed::from(market.authority.as_ref()),
            Seed::from(market_seed_bytes.as_ref()),
            Seed::from(market_bump.as_ref()),
        ];

        for (mint, vault, vault_balance) in [
            (
                self.mint_y.to_account_view(),
                self.vault_y.to_account_view(),
                self.vault_y.amount(),
            ),
            (
                self.mint_n.to_account_view(),
                self.vault_n.to_account_view(),
                self.vault_n.amount(),
            ),
        ] {
            self.token_program
                .burn(vault, mint, market.to_account_view(), vault_balance)
                .invoke_signed(&market_signer_seeds)?;

            self.token_program
                .close_account(
                    vault,
                    self.authority.to_account_view(),
                    market.to_account_view(),
                )
                .invoke_signed(&market_signer_seeds)?;
        }

        if outcome != Outcome::Refunded {
            let fee_amount = fee_from_pot(self.pot.amount(), market.fee_bps())
                .map_err(FloatingOddsExchangeError::from)?;

            let market_seed: u64 = market.seed.into();
            let market_seed_bytes = market_seed.to_le_bytes();
            let market_bump = [market.bump];

            let market_signer_seeds = [
                Seed::from(Market::SEED_PREFIX as &[u8]),
                Seed::from(market.authority.as_ref()),
                Seed::from(market_seed_bytes.as_ref()),
                Seed::from(market_bump.as_ref()),
            ];

            self.token_program
                .transfer_checked(
                    self.pot.to_account_view(),
                    self.pot_mint.to_account_view(),
                    self.payout.to_account_view(),
                    market.to_account_view(),
                    fee_amount,
                    self.pot_mint.decimals(),
                )
                .invoke_signed(&market_signer_seeds)?;
        }

        emit_cpi!(MarketResolved {
            market: *market.address(),
            // memcpy bug
            // outcome: outcome.into(),
            slot: slot.into(),
        })?;

        Ok(())
    }
}
