extern crate std;

mod errors;
mod instructions;

use {
    alloc::vec,
    floating_odds_exchange_client::{
        find_market_address, find_mint_n_address, find_mint_y_address, state::Market,
        BuyInstruction, ClaimInstruction, FloatingOddsExchangeError, InitializeInstruction,
        LaunchInstruction, PauseInstruction, ResolveInstruction,
    },
    quasar_lang::client::DynString,
    quasar_metadata::METADATA_PROGRAM_ID,
    quasar_svm::{
        solana_sdk_ids, system_program,
        token::{
            create_keyed_associated_token_account, create_keyed_mint_account,
            create_keyed_system_account, Mint, TokenAccount,
        },
        Account, Instruction, ProgramError, QuasarSvm, SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        SPL_TOKEN_PROGRAM_ID,
    },
    solana_address::Address,
    solana_program_pack::Pack,
    std::{path::Path, vec::Vec},
};

// Deterministic keys — avoid Address::new_unique() (order-dependent across tests).
const AUTHORITY: Address = Address::new_from_array([1; 32]);
const BUYER: Address = Address::new_from_array([2; 32]);
const POT_MINT: Address = Address::new_from_array([3; 32]);
const STRANGER: Address = Address::new_from_array([4; 32]);
const OTHER_MINT: Address = Address::new_from_array([5; 32]);

const SEED: u64 = 42;
const FEE_BPS: u16 = 100; // 1%
const POT_DECIMALS: u8 = 6;
const SIDE_LIQUIDITY: u64 = 1_000_000;
const POT_LIQUIDITY: u64 = 500_000_000_000;
const AUTHORITY_LAMPORTS: u64 = 100_000_000_000;
const BUYER_POT_BALANCE: u64 = 10_000_000_000_000;

fn setup() -> QuasarSvm {
    let manifest_dir = Path::new(env!("CARGO_MANIFEST_DIR"));
    let elf =
        std::fs::read(manifest_dir.join("../../target/deploy/floating_odds_exchange.so")).unwrap();
    let mpl = std::fs::read(manifest_dir.join("src/tests/fixtures/mpl_token_metadata.so"))
        .expect("missing src/tests/fixtures/mpl_token_metadata.so");
    QuasarSvm::new()
        .with_program(&program_id(), &elf)
        .with_token_program()
        .with_associated_token_program()
        .with_program(&METADATA_PROGRAM_ID, &mpl)
}

fn program_id() -> Address {
    Address::from(crate::ID.to_bytes())
}

fn event_authority() -> Address {
    Address::find_program_address(&[b"__event_authority"], &program_id()).0
}

fn market_pdas() -> MarketPdas {
    let (market, market_bump) = find_market_address(&AUTHORITY, SEED, &program_id());
    let (mint_y, mint_y_bump) = find_mint_y_address(&market, &program_id());
    let (mint_n, mint_n_bump) = find_mint_n_address(&market, &program_id());
    MarketPdas {
        market,
        market_bump,
        mint_y,
        mint_y_bump,
        mint_n,
        mint_n_bump,
        metadata_y: metadata_pda(&mint_y),
        metadata_n: metadata_pda(&mint_n),
        pot: create_keyed_associated_token_account(&market, &POT_MINT, 0).address,
        vault_y: create_keyed_associated_token_account(&market, &mint_y, 0).address,
        vault_n: create_keyed_associated_token_account(&market, &mint_n, 0).address,
        authority_pot_ata: create_keyed_associated_token_account(&AUTHORITY, &POT_MINT, 0).address,
        buyer_pot_ata: create_keyed_associated_token_account(&BUYER, &POT_MINT, 0).address,
        buyer_y_ata: create_keyed_associated_token_account(&BUYER, &mint_y, 0).address,
        buyer_n_ata: create_keyed_associated_token_account(&BUYER, &mint_n, 0).address,
        payout_ata: create_keyed_associated_token_account(&AUTHORITY, &POT_MINT, 0).address,
    }
}

fn metadata_pda(mint: &Address) -> Address {
    Address::find_program_address(
        &[b"metadata", METADATA_PROGRAM_ID.as_ref(), mint.as_ref()],
        &METADATA_PROGRAM_ID,
    )
    .0
}

struct MarketPdas {
    market: Address,
    market_bump: u8,
    mint_y: Address,
    mint_y_bump: u8,
    mint_n: Address,
    mint_n_bump: u8,
    metadata_y: Address,
    metadata_n: Address,
    pot: Address,
    vault_y: Address,
    vault_n: Address,
    authority_pot_ata: Address,
    buyer_pot_ata: Address,
    buyer_y_ata: Address,
    buyer_n_ata: Address,
    payout_ata: Address,
}

fn pot_mint(freeze_authority: Option<Address>) -> Account {
    create_keyed_mint_account(
        &POT_MINT,
        &Mint {
            mint_authority: Some(AUTHORITY).into(),
            supply: BUYER_POT_BALANCE + POT_LIQUIDITY,
            decimals: POT_DECIMALS,
            is_initialized: true,
            freeze_authority: freeze_authority.into(),
        },
    )
}

fn token_amount(account: &Account) -> u64 {
    TokenAccount::unpack(&account.data)
        .expect("token account")
        .amount
}

fn mint_supply(account: &Account) -> u64 {
    Mint::unpack(&account.data).expect("mint account").supply
}

fn decode_market(data: &[u8]) -> Market {
    wincode::deserialize(data).expect("market account")
}

fn expect_custom_error(result: &quasar_svm::ExecutionResult, error: FloatingOddsExchangeError) {
    result.assert_error(ProgramError::Custom(error as u32));
}

fn initialize_ix(
    pdas: &MarketPdas,
    name: &str,
    symbol: &str,
    uri: &str,
    fee_bps: u16,
) -> Instruction {
    InitializeInstruction {
        authority: AUTHORITY,
        market: pdas.market,
        pot_mint: POT_MINT,
        mint_y: pdas.mint_y,
        mint_n: pdas.mint_n,
        metadata_y: pdas.metadata_y,
        metadata_n: pdas.metadata_n,
        system_program: system_program::ID,
        token_program: SPL_TOKEN_PROGRAM_ID,
        associated_token_program: SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        metadata_program: METADATA_PROGRAM_ID,
        rent: solana_sdk_ids::sysvar::rent::ID,
        event_authority: event_authority(),
        program: program_id(),
        seed: SEED,
        fee_bps,
        name: DynString::new(name),
        symbol: DynString::new(symbol),
        uri: DynString::new(uri),
    }
    .into()
}

fn initialize_accounts(pdas: &MarketPdas) -> Vec<Account> {
    vec![
        create_keyed_system_account(&AUTHORITY, AUTHORITY_LAMPORTS),
        create_keyed_system_account(&pdas.market, 0),
        pot_mint(None),
        create_keyed_system_account(&pdas.mint_y, 0),
        create_keyed_system_account(&pdas.mint_n, 0),
        create_keyed_system_account(&pdas.metadata_y, 0),
        create_keyed_system_account(&pdas.metadata_n, 0),
        create_keyed_system_account(&event_authority(), 0),
        create_keyed_associated_token_account(&AUTHORITY, &POT_MINT, POT_LIQUIDITY),
    ]
}

fn launch_ix(pdas: &MarketPdas, side: u64, pot: u64) -> Instruction {
    LaunchInstruction {
        authority: AUTHORITY,
        market: pdas.market,
        pot_mint: POT_MINT,
        mint_y: pdas.mint_y,
        mint_n: pdas.mint_n,
        pot: pdas.pot,
        vault_y: pdas.vault_y,
        vault_n: pdas.vault_n,
        authority_pot_mint_token_account: pdas.authority_pot_ata,
        system_program: system_program::ID,
        pot_mint_token_program: SPL_TOKEN_PROGRAM_ID,
        token_program: SPL_TOKEN_PROGRAM_ID,
        associated_token_program: SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        event_authority: event_authority(),
        program: program_id(),
        amount_y: side,
        amount_n: pot,
    }
    .into()
}

fn buy_ix(
    pdas: &MarketPdas,
    buy_yes: bool,
    in_amount: u64,
    out_amount: u64,
    exact_in: bool,
    amount_with_slippage: u64,
) -> Instruction {
    let (buy_mint, other_mint, user_buy_ata) = if buy_yes {
        (pdas.mint_y, pdas.mint_n, pdas.buyer_y_ata)
    } else {
        (pdas.mint_n, pdas.mint_y, pdas.buyer_n_ata)
    };
    BuyInstruction {
        buyer: BUYER,
        market: pdas.market,
        pot_mint: POT_MINT,
        buy_mint,
        other_mint,
        pot: pdas.pot,
        user_pot_mint_token_account: pdas.buyer_pot_ata,
        user_buy_mint_token_account: user_buy_ata,
        system_program: system_program::ID,
        pot_mint_token_program: SPL_TOKEN_PROGRAM_ID,
        buy_mint_token_program: SPL_TOKEN_PROGRAM_ID,
        associated_token_program: SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        event_authority: event_authority(),
        program: program_id(),
        in_amount,
        out_amount,
        exact_in,
        amount_with_slippage,
    }
    .into()
}

fn pause_ix(pdas: &MarketPdas, authority: Address) -> Instruction {
    PauseInstruction {
        authority,
        market: pdas.market,
        event_authority: event_authority(),
        program: program_id(),
    }
    .into()
}

fn resolve_ix(pdas: &MarketPdas, outcome: u8) -> Instruction {
    ResolveInstruction {
        authority: AUTHORITY,
        market: pdas.market,
        pot_mint: POT_MINT,
        mint_y: pdas.mint_y,
        mint_n: pdas.mint_n,
        pot: pdas.pot,
        vault_y: pdas.vault_y,
        vault_n: pdas.vault_n,
        payout: pdas.payout_ata,
        system_program: system_program::ID,
        token_program: SPL_TOKEN_PROGRAM_ID,
        event_authority: event_authority(),
        program: program_id(),
        outcome,
    }
    .into()
}

fn claim_ix(pdas: &MarketPdas, include_y: bool, include_n: bool) -> Instruction {
    let none = program_id();
    ClaimInstruction {
        claimer: BUYER,
        market: pdas.market,
        pot_mint: POT_MINT,
        mint_y: pdas.mint_y,
        mint_n: pdas.mint_n,
        pot: pdas.pot,
        claimer_pot_mint_token_account: pdas.buyer_pot_ata,
        claimer_mint_y_token_account: if include_y { pdas.buyer_y_ata } else { none },
        claimer_mint_n_token_account: if include_n { pdas.buyer_n_ata } else { none },
        pot_mint_token_program: SPL_TOKEN_PROGRAM_ID,
        bet_mint_token_program: SPL_TOKEN_PROGRAM_ID,
        event_authority: event_authority(),
        program: program_id(),
    }
    .into()
}

/// Run initialize and return updated accounts.
fn do_initialize(svm: &mut QuasarSvm, pdas: &MarketPdas) -> Vec<Account> {
    let result = svm.process_instruction(
        &initialize_ix(pdas, "FOX", "FOX", "https://example.com/fox.json", FEE_BPS),
        &initialize_accounts(pdas),
    );
    result.assert_success();
    result.accounts
}

/// Initialize then launch with default liquidity.
fn do_launch(svm: &mut QuasarSvm, pdas: &MarketPdas) -> Vec<Account> {
    let accounts = do_initialize(svm, pdas);
    let pot = create_keyed_associated_token_account(&pdas.market, &POT_MINT, 0);
    let vault_y = create_keyed_associated_token_account(&pdas.market, &pdas.mint_y, 0);
    let vault_n = create_keyed_associated_token_account(&pdas.market, &pdas.mint_n, 0);
    let mut accounts = accounts;
    accounts.push(pot);
    accounts.push(vault_y);
    accounts.push(vault_n);

    let result =
        svm.process_instruction(&launch_ix(pdas, SIDE_LIQUIDITY, POT_LIQUIDITY), &accounts);
    result.assert_success();
    result.accounts
}

/// Launched market plus a funded buyer ready to trade.
fn do_launched_with_buyer(svm: &mut QuasarSvm, pdas: &MarketPdas) -> Vec<Account> {
    let mut accounts = do_launch(svm, pdas);
    accounts.push(create_keyed_system_account(&BUYER, AUTHORITY_LAMPORTS));
    accounts.push(create_keyed_associated_token_account(
        &BUYER,
        &POT_MINT,
        BUYER_POT_BALANCE,
    ));
    accounts.push(create_keyed_associated_token_account(
        &BUYER,
        &pdas.mint_y,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &BUYER,
        &pdas.mint_n,
        0,
    ));
    accounts
}
