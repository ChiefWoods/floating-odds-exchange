use quasar_lang::{cpi::Seed, prelude::*, sysvars::Sysvar};
use quasar_spl::prelude::*;

use crate::{
    errors::FloatingOddsExchangeError,
    events::MarketLaunched,
    state::{Market, MintN, MintY},
    EventAuthority, FloatingOddsExchange,
};

#[derive(Accounts)]
pub struct Launch {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        mut,
        constraints(market.is_launched.is_false()) @ FloatingOddsExchangeError::MarketAlreadyLaunched,
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
        init(idempotent),
        payer = authority,
        associated_token(
            mint = pot_mint,
            authority = market,
            token_program = pot_mint_token_program,
        )
    )]
    pub pot: InterfaceAccount<Token>,

    #[account(
        mut,
        init(idempotent),
        payer = authority,
        associated_token(
            mint = mint_y,
            authority = market,
            token_program = token_program,
        )
    )]
    pub vault_y: Account<Token>,

    #[account(
        mut,
        init(idempotent),
        payer = authority,
        associated_token(
            mint = mint_n,
            authority = market,
            token_program = token_program,
        )
    )]
    pub vault_n: Account<Token>,

    #[account(mut)]
    pub authority_pot_mint_token_account: InterfaceAccount<Token>,

    pub system_program: Program<SystemProgram>,
    pub pot_mint_token_program: Interface<TokenInterface>,
    /// CHECK: aliases `pot_mint_token_program` when the pot uses classic SPL Token
    #[account(dup)]
    pub token_program: Program<TokenProgram>,
    pub associated_token_program: Program<AssociatedTokenProgram>,
    pub event_authority: EventAuthority,
    pub program: Program<FloatingOddsExchange>,
}

impl Launch {
    /// 1. Validate equal nonzero `side_mint_amount` and `pot_mint_amount`.
    /// 2. Transfer backing liquidity into pot.
    /// 3. Mint initial supply into Market Y/N vaults.
    /// 4. Mark market as launched.
    #[inline(always)]
    pub fn handler(
        &mut self,
        side_mint_amount: u64,
        pot_mint_amount: u64,
    ) -> Result<(), ProgramError> {
        let Clock { slot, .. } = Clock::get()?;

        let market = &mut self.market;

        MintY::seeds(market.address()).verify_existing(self.mint_y.address(), &crate::ID)?;
        MintN::seeds(market.address()).verify_existing(self.mint_n.address(), &crate::ID)?;

        require!(
            side_mint_amount > 0 && pot_mint_amount > 0,
            FloatingOddsExchangeError::InvalidAmount
        );

        self.pot_mint_token_program
            .transfer_checked(
                self.authority_pot_mint_token_account.to_account_view(),
                self.pot_mint.to_account_view(),
                self.pot.to_account_view(),
                self.authority.to_account_view(),
                pot_mint_amount,
                self.pot_mint.decimals(),
            )
            .invoke()?;

        let market_seed: u64 = market.seed.into();
        let market_seed_bytes = market_seed.to_le_bytes();
        let market_bump = [market.bump];

        let market_signer_seeds = [
            Seed::from(Market::SEED_PREFIX as &[u8]),
            Seed::from(market.authority.as_ref()),
            Seed::from(market_seed_bytes.as_ref()),
            Seed::from(market_bump.as_ref()),
        ];

        for (mint, vault) in [
            (
                self.mint_y.to_account_view(),
                self.vault_y.to_account_view(),
            ),
            (
                self.mint_n.to_account_view(),
                self.vault_n.to_account_view(),
            ),
        ] {
            self.token_program
                .mint_to(mint, vault, market.to_account_view(), side_mint_amount)
                .invoke_signed(&market_signer_seeds)?;
        }

        self.market.is_launched.set(true);

        emit_cpi!(MarketLaunched {
            market: *self.market.address(),
            slot: slot.into(),
        })?;

        Ok(())
    }
}
