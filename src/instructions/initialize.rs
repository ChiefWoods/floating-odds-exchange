use basis_points::BasisPoints;
use quasar_lang::{cpi::Seed, prelude::*, sysvars::Sysvar};
use quasar_metadata::prelude::*;
use quasar_spl::prelude::*;

use crate::{
    constants::{NAME_MAX_LEN, SYMBOL_MAX_LEN},
    errors::FloatingOddsExchangeError,
    events::MarketInitialized,
    state::{Market, MarketInner, MintN, MintY, Outcome},
    EventAuthority, FloatingOddsExchange,
};

#[derive(Accounts)]
#[instruction(seed: u64)]
pub struct Initialize {
    #[account(mut)]
    pub authority: Signer,

    #[account(
        init,
        payer = authority,
        address = Market::seeds(authority.address(), seed)
    )]
    pub market: Account<Market>,

    pub pot_mint: InterfaceAccount<Mint>,

    #[account(
        init,
        payer = authority,
        mint(
            authority = market,
            decimals = 0,
            freeze_authority = None,
            token_program = token_program,
        ),
        address = MintY::seeds(market.address()),
    )]
    pub mint_y: Account<Mint>,

    #[account(
        init,
        payer = authority,
        mint(
            authority = market,
            decimals = 0,
            freeze_authority = None,
            token_program = token_program,
        ),
        address = MintN::seeds(market.address()),
    )]
    pub mint_n: Account<Mint>,

    #[account(mut)]
    pub metadata_y: UncheckedAccount,

    #[account(mut)]
    pub metadata_n: UncheckedAccount,

    pub system_program: Program<SystemProgram>,
    pub token_program: Program<TokenProgram>,
    pub associated_token_program: Program<AssociatedTokenProgram>,
    pub metadata_program: Program<MetadataProgram>,
    pub rent: quasar_lang::accounts::Sysvar<Rent>,
    pub event_authority: EventAuthority,
    pub program: Program<FloatingOddsExchange>,
}

impl Initialize {
    /// 1. Validate fee_bps.
    /// 2. Validate token metadata.
    /// 3. Validate pot mint.
    /// 4. Create Metaplex metadata for YES and NO mints (market PDA signs as mint authority).
    /// 5. Create a new Market account.
    #[inline(never)]
    pub fn handler(
        &mut self,
        bumps: &InitializeBumps,
        seed: u64,
        fee_bps: u16,
        name: &str,
        symbol: &str,
        uri: &str,
    ) -> Result<(), ProgramError> {
        let Clock { slot, .. } = Clock::get()?;

        let fee_bps = BasisPoints::new(fee_bps)?.into();

        require!(
            name.len() > 0,
            FloatingOddsExchangeError::InvalidMintMetadata
        );
        require!(
            symbol.len() > 0,
            FloatingOddsExchangeError::InvalidMintMetadata
        );
        require!(
            uri.len() > 0,
            FloatingOddsExchangeError::InvalidMintMetadata
        );

        require!(
            self.pot_mint.freeze_authority().is_none(),
            FloatingOddsExchangeError::PotMintFreezable
        );

        self.create_outcome_metadata(bumps, seed, true, name, symbol, uri)?;
        self.create_outcome_metadata(bumps, seed, false, name, symbol, uri)?;

        self.market.set_inner(MarketInner {
            seed,
            authority: *self.authority.address(),
            payout: *self.authority.address(),
            pot_mint: *self.pot_mint.address(),
            precision: 10_u64.pow(self.pot_mint.decimals() as u32),
            outcome: Outcome::Undecided.into(),
            fee_bps,
            mint_y_bump: bumps.mint_y,
            mint_n_bump: bumps.mint_n,
            bump: bumps.market,
            is_launched: false,
            is_paused: false,
        });

        emit_cpi!(MarketInitialized {
            authority: *self.authority.address(),
            market: *self.market.address(),
            seed,
            slot: slot.into(),
        })?;

        Ok(())
    }

    /// Isolated so the Metaplex `CpiDynamic<_, 512>` buffer does not share a
    /// frame with mint `init` and market `set_inner` (SBF 4 KiB stack).
    #[inline(never)]
    fn create_outcome_metadata(
        &self,
        bumps: &InitializeBumps,
        seed: u64,
        is_yes: bool,
        name: &str,
        symbol: &str,
        uri: &str,
    ) -> Result<(), ProgramError> {
        let market_seed_bytes = seed.to_le_bytes();
        let market_bump = [bumps.market];
        let market_signer_seeds = [
            Seed::from(Market::SEED_PREFIX as &[u8]),
            Seed::from(self.authority.address().as_ref()),
            Seed::from(market_seed_bytes.as_ref()),
            Seed::from(market_bump.as_ref()),
        ];

        let (metadata, mint, name_prefix, symbol_prefix) = if is_yes {
            (
                self.metadata_y.to_account_view(),
                self.mint_y.to_account_view(),
                b"Y".as_slice(),
                b"y".as_slice(),
            )
        } else {
            (
                self.metadata_n.to_account_view(),
                self.mint_n.to_account_view(),
                b"N".as_slice(),
                b"n".as_slice(),
            )
        };

        self.metadata_program
            .create_metadata_accounts_v3(
                metadata,
                mint,
                self.market.to_account_view(),
                self.authority.to_account_view(),
                self.market.to_account_view(),
                self.system_program.to_account_view(),
                self.rent.to_account_view(),
                prefix_meta(&mut [0u8; NAME_MAX_LEN], name_prefix, name)?,
                prefix_meta(&mut [0u8; SYMBOL_MAX_LEN], symbol_prefix, symbol)?,
                uri,
                0,
                true,
                false,
            )?
            .invoke_signed(&market_signer_seeds)?;

        Ok(())
    }
}

/// Write `prefix + value` into `buf` and return the resulting `&str`.
#[inline(always)]
fn prefix_meta<'a>(buf: &'a mut [u8], prefix: &[u8], value: &str) -> Result<&'a str, ProgramError> {
    let total = prefix
        .len()
        .checked_add(value.len())
        .ok_or(FloatingOddsExchangeError::ArithmeticOverflow)?;

    if total > buf.len() {
        return Err(FloatingOddsExchangeError::InvalidMetadata.into());
    }

    buf[..prefix.len()].copy_from_slice(prefix);
    buf[prefix.len()..total].copy_from_slice(value.as_bytes());
    core::str::from_utf8(&buf[..total])
        .map_err(|_| FloatingOddsExchangeError::InvalidUtf8String.into())
}
