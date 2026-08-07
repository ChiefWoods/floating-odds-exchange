use quasar_lang::{cpi::Seed, prelude::*, sysvars::Sysvar};
use quasar_spl::prelude::*;

use crate::{
    errors::FloatingOddsExchangeError,
    events::BetCreated,
    math::{quote_exact_input, quote_exact_output},
    state::{Market, MintN, MintY, Outcome},
    EventAuthority, FloatingOddsExchange,
};

#[derive(Accounts)]
pub struct Buy {
    #[account(mut)]
    pub buyer: Signer,

    #[account(
        mut,
        constraints(market.is_launched.is_true()) @ FloatingOddsExchangeError::MarketNotLaunched,
        constraints(market.is_paused.is_false()) @ FloatingOddsExchangeError::MarketPaused,
        constraints(market.outcome == u8::from(Outcome::Undecided)) @ FloatingOddsExchangeError::MarketAlreadyResolved,
        has_one(pot_mint) @ FloatingOddsExchangeError::InvalidMint,
    )]
    pub market: Account<Market>,

    pub pot_mint: InterfaceAccount<Mint>,

    /// Outcome mint being purchased (market YES or NO PDA).
    #[account(mut)]
    pub buy_mint: Account<Mint>,

    /// Opposite outcome mint (the other of YES/NO).
    #[account(mut)]
    pub other_mint: Account<Mint>,

    #[account(
        mut,
        associated_token(
            mint = pot_mint,
            authority = market,
            token_program = pot_mint_token_program
        )
    )]
    pub pot: InterfaceAccount<Token>,

    /// not validated, guarded by CPI
    #[account(mut)]
    pub user_pot_mint_token_account: InterfaceAccount<Token>,

    #[account(
        init(idempotent),
        payer = buyer,
        associated_token(
            mint = buy_mint,
            authority = buyer,
            token_program = buy_mint_token_program,
            ata_program = associated_token_program,
        )
    )]
    pub user_buy_mint_token_account: Account<Token>,

    pub system_program: Program<SystemProgram>,
    pub pot_mint_token_program: Interface<TokenInterface>,
    /// CHECK: aliases `pot_mint_token_program` when both sides use classic SPL Token
    #[account(dup)]
    pub buy_mint_token_program: Program<TokenProgram>,
    pub associated_token_program: Program<AssociatedTokenProgram>,
    pub event_authority: EventAuthority,
    pub program: Program<FloatingOddsExchange>,
}

impl Buy {
    /// 1. Validate `buy_mint` / `other_mint` are the market YES/NO pair (either order).
    /// 2. Get quote amount from those mint supplies.
    /// 3. Validate slippage not exceeded.
    /// 4. Mint quoted out amount of `buy_mint` to user.
    /// 5. Transfer quoted in_amount from user to pot.
    #[inline(never)]
    pub fn handler(
        &mut self,
        in_amount: u64,
        out_amount: u64,
        exact_in: bool,
        amount_with_slippage: u64,
    ) -> Result<(), ProgramError> {
        let Clock { slot, .. } = Clock::get()?;
        let market = &mut self.market;

        Self::verify_outcome_mint_pair(
            market.address(),
            self.buy_mint.address(),
            self.other_mint.address(),
        )?;

        let supply_buy = self.buy_mint.supply();
        let supply_other = self.other_mint.supply();

        let (quoted_in_amount, quoted_out_amount) = if exact_in {
            let (quoted_out, quoted_in) =
                quote_exact_input(supply_buy, supply_other, in_amount, market.precision())?;
            require!(
                quoted_out >= amount_with_slippage,
                FloatingOddsExchangeError::SlippageExceeded
            );
            (quoted_in, quoted_out)
        } else {
            let quoted_in =
                quote_exact_output(supply_buy, supply_other, out_amount, market.precision())?;
            require!(
                quoted_in <= amount_with_slippage,
                FloatingOddsExchangeError::SlippageExceeded
            );
            (quoted_in, out_amount)
        };

        let market_seed: u64 = market.seed.into();
        let market_seed_bytes = market_seed.to_le_bytes();
        let market_bump = [market.bump];

        let market_signer_seeds = [
            Seed::from(Market::SEED_PREFIX as &[u8]),
            Seed::from(market.authority.as_ref()),
            Seed::from(market_seed_bytes.as_ref()),
            Seed::from(market_bump.as_ref()),
        ];

        self.buy_mint_token_program
            .mint_to(
                self.buy_mint.to_account_view(),
                self.user_buy_mint_token_account.to_account_view(),
                market.to_account_view(),
                quoted_out_amount,
            )
            .invoke_signed(&market_signer_seeds)?;

        self.pot_mint_token_program
            .transfer_checked(
                self.user_pot_mint_token_account.to_account_view(),
                self.pot_mint.to_account_view(),
                self.pot.to_account_view(),
                self.buyer.to_account_view(),
                quoted_in_amount,
                self.pot_mint.decimals(),
            )
            .invoke()?;

        emit_cpi!(BetCreated {
            market: *self.market.address(),
            buyer: *self.buyer.address(),
            in_amount: quoted_in_amount,
            out_amount: quoted_out_amount,
            slot: slot.into(),
        })?;

        Ok(())
    }

    /// `buy_mint` must be YES or NO; `other_mint` must be the opposite.
    #[inline(always)]
    fn verify_outcome_mint_pair(
        market: &Address,
        buy_mint: &Address,
        other_mint: &Address,
    ) -> Result<(), ProgramError> {
        let mint_y = MintY::seeds(market);
        let mint_n = MintN::seeds(market);

        if mint_y.verify_existing(buy_mint, &crate::ID).is_ok() {
            mint_n.verify_existing(other_mint, &crate::ID)?;
            Ok(())
        } else if mint_n.verify_existing(buy_mint, &crate::ID).is_ok() {
            mint_y.verify_existing(other_mint, &crate::ID)?;
            Ok(())
        } else {
            Err(FloatingOddsExchangeError::InvalidMint.into())
        }
    }
}
