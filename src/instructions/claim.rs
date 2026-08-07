use quasar_lang::{cpi::Seed, prelude::*, sysvars::Sysvar};
use quasar_spl::prelude::*;
use solana_math::SafeMath;

use crate::{
    errors::FloatingOddsExchangeError,
    events::WinningsClaimed,
    math::claims_from_pot,
    state::{Market, MintN, MintY, Outcome},
    EventAuthority, FloatingOddsExchange,
};

#[derive(Accounts)]
pub struct Claim {
    #[account(mut)]
    pub claimer: Signer,

    #[account(
        mut,
        constraints(market.outcome != u8::from(Outcome::Undecided)) @ FloatingOddsExchangeError::MarketNotResolved,
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
            token_program = pot_mint_token_program
        )
    )]
    pub pot: InterfaceAccount<Token>,

    /// not validated, guarded by CPI
    #[account(mut)]
    pub claimer_pot_mint_token_account: InterfaceAccount<Token>,

    /// CHECK: None uses the program-id sentinel; may alias the other optional side
    #[account(mut, dup)]
    pub claimer_mint_y_token_account: Option<Account<Token>>,

    /// CHECK: None uses the program-id sentinel; may alias the other optional side
    #[account(mut, dup)]
    pub claimer_mint_n_token_account: Option<Account<Token>>,

    pub pot_mint_token_program: Interface<TokenInterface>,
    /// CHECK: aliases `pot_mint_token_program` when both sides use classic SPL Token
    #[account(dup)]
    pub bet_mint_token_program: Program<TokenProgram>,
    pub event_authority: EventAuthority,
    /// CHECK: may alias Option `None` sentinels that use the program id
    #[account(dup)]
    pub program: Program<FloatingOddsExchange>,
}

impl Claim {
    /// 1. Validate market is not `Undecided`.
    /// 2. Verify mints.
    ///
    /// If market outcome is `Yes` or `No`:
    /// 3a. Calculate redeemable amount for the winning side.
    ///
    /// If market outcome is `Refunded`:
    /// 3b. Calculate refundable amount using both mint supplies.
    ///
    /// 4. Burn each passed claimer token account.
    /// 5. Close each passed token account.
    /// 6. Transfer redeemable/refundable pot mint to claimer.
    #[inline(always)]
    pub fn handler(&mut self) -> Result<(), ProgramError> {
        let Clock { slot, .. } = Clock::get()?;
        let market = &self.market;
        let outcome = Outcome::try_from(market.outcome)?;

        MintY::seeds(market.address()).verify_existing(self.mint_y.address(), &crate::ID)?;
        MintN::seeds(market.address()).verify_existing(self.mint_n.address(), &crate::ID)?;

        let pot_amount = self.pot.amount();

        let claim_amount = match outcome {
            Outcome::Yes => {
                let token_account = self
                    .claimer_mint_y_token_account
                    .as_ref()
                    .ok_or(FloatingOddsExchangeError::TokenAccountNotInitialized)?;
                claims_from_pot(token_account.amount(), self.mint_y.supply(), pot_amount)?
            }
            Outcome::No => {
                let token_account = self
                    .claimer_mint_n_token_account
                    .as_ref()
                    .ok_or(FloatingOddsExchangeError::TokenAccountNotInitialized)?;
                claims_from_pot(token_account.amount(), self.mint_n.supply(), pot_amount)?
            }
            Outcome::Refunded => {
                let amount_y = Self::token_amount(self.claimer_mint_y_token_account.as_ref());
                let amount_n = Self::token_amount(self.claimer_mint_n_token_account.as_ref());

                claims_from_pot(
                    amount_y.safe_add(amount_n)?,
                    self.mint_y.supply().safe_add(self.mint_n.supply())?,
                    pot_amount,
                )?
            }
            Outcome::Undecided => return Err(FloatingOddsExchangeError::MarketNotResolved.into()),
        };

        let claimer = self.claimer.to_account_view();

        if let Some(token_account) = self.claimer_mint_y_token_account.as_ref() {
            self.bet_mint_token_program
                .burn(
                    token_account.to_account_view(),
                    self.mint_y.to_account_view(),
                    claimer,
                    token_account.amount(),
                )
                .invoke()?;

            self.bet_mint_token_program
                .close_account(token_account.to_account_view(), claimer, claimer)
                .invoke()?;
        }
        if let Some(token_account) = self.claimer_mint_n_token_account.as_ref() {
            self.bet_mint_token_program
                .burn(
                    token_account.to_account_view(),
                    self.mint_n.to_account_view(),
                    claimer,
                    token_account.amount(),
                )
                .invoke()?;

            self.bet_mint_token_program
                .close_account(token_account.to_account_view(), claimer, claimer)
                .invoke()?;
        }

        let market_seed: u64 = market.seed.into();
        let market_seed_bytes = market_seed.to_le_bytes();
        let market_bump = [market.bump];

        let market_signer_seeds = [
            Seed::from(Market::SEED_PREFIX as &[u8]),
            Seed::from(market.authority.as_ref()),
            Seed::from(market_seed_bytes.as_ref()),
            Seed::from(market_bump.as_ref()),
        ];

        self.pot_mint_token_program
            .transfer_checked(
                self.pot.to_account_view(),
                self.pot_mint.to_account_view(),
                self.claimer_pot_mint_token_account.to_account_view(),
                market.to_account_view(),
                claim_amount,
                self.pot_mint.decimals(),
            )
            .invoke_signed(&market_signer_seeds)?;

        emit_cpi!(WinningsClaimed {
            market: *market.address(),
            claimer: *self.claimer.address(),
            amount: claim_amount,
            slot: slot.into(),
        })?;

        Ok(())
    }

    #[inline(always)]
    fn token_amount(token_account: Option<&Account<Token>>) -> u64 {
        token_account.map(|ta| ta.amount()).unwrap_or(0)
    }
}
