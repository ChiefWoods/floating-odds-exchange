//! `ArithmeticOverflow` is not covered by a dedicated fixture: it only surfaces
//! from pathological checked-math edge cases, not a stable instruction path.

use super::*;

#[test]
fn rejects_initialize_with_empty_metadata() {
    let mut svm = setup();
    let pdas = market_pdas();
    let result = svm.process_instruction(
        &initialize_ix(&pdas, "", "FOX", "https://example.com", FEE_BPS),
        &initialize_accounts(&pdas),
    );
    expect_custom_error(&result, FloatingOddsExchangeError::InvalidMintMetadata);
}

#[test]
fn rejects_initialize_when_pot_mint_is_freezable() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = initialize_accounts(&pdas);
    // Replace pot mint with a freezable one.
    accounts.retain(|a| a.address != POT_MINT);
    accounts.push(pot_mint(Some(AUTHORITY)));

    let result = svm.process_instruction(
        &initialize_ix(&pdas, "FOX", "FOX", "https://example.com", FEE_BPS),
        &accounts,
    );
    expect_custom_error(&result, FloatingOddsExchangeError::PotMintFreezable);
}

#[test]
fn rejects_initialize_when_prefixed_name_exceeds_metadata_buffer() {
    let mut svm = setup();
    let pdas = market_pdas();
    // NAME_MAX_LEN is 32; prefix_meta adds 1 byte ("Y"/"N"), so 32-byte name → InvalidMetadata.
    let name = "abcdefghijklmnopqrstuvwxyz012345"; // 32 bytes
    assert_eq!(name.len(), 32);
    let result = svm.process_instruction(
        &initialize_ix(&pdas, name, "FOX", "https://example.com", FEE_BPS),
        &initialize_accounts(&pdas),
    );
    expect_custom_error(&result, FloatingOddsExchangeError::InvalidMetadata);
}

#[test]
fn rejects_launch_when_amounts_are_zero() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_initialize(&mut svm, &pdas);
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &POT_MINT,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_y,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_n,
        0,
    ));

    let result = svm.process_instruction(&launch_ix(&pdas, 0, POT_LIQUIDITY), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::InvalidAmount);
}

#[test]
fn rejects_second_launch() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launch(&mut svm, &pdas);
    let result =
        svm.process_instruction(&launch_ix(&pdas, SIDE_LIQUIDITY, POT_LIQUIDITY), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketAlreadyLaunched);
}

#[test]
fn rejects_launch_from_non_authority() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_initialize(&mut svm, &pdas);
    accounts.push(create_keyed_system_account(&STRANGER, AUTHORITY_LAMPORTS));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &POT_MINT,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_y,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_n,
        0,
    ));

    let ix: Instruction = LaunchInstruction {
        authority: STRANGER,
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
        amount_y: SIDE_LIQUIDITY,
        amount_n: POT_LIQUIDITY,
    }
    .into();

    let result = svm.process_instruction(&ix, &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::UnauthorizedAuthority);
}

#[test]
fn rejects_buy_before_launch() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_initialize(&mut svm, &pdas);
    accounts.push(create_keyed_system_account(&BUYER, AUTHORITY_LAMPORTS));
    accounts.push(create_keyed_associated_token_account(
        &BUYER,
        &POT_MINT,
        BUYER_POT_BALANCE,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &POT_MINT,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &BUYER,
        &pdas.mint_y,
        0,
    ));

    let result =
        svm.process_instruction(&buy_ix(&pdas, true, 0, 1_000, false, u64::MAX), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketNotLaunched);
}

#[test]
fn rejects_buy_when_paused() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launched_with_buyer(&mut svm, &pdas);
    let paused = svm.process_instruction(&pause_ix(&pdas, AUTHORITY), &accounts);
    paused.assert_success();
    accounts = paused.accounts;

    let result =
        svm.process_instruction(&buy_ix(&pdas, true, 0, 1_000, false, u64::MAX), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketPaused);
}

#[test]
fn rejects_buy_after_resolve() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launched_with_buyer(&mut svm, &pdas);
    let resolved = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    resolved.assert_success();
    accounts = resolved.accounts;

    // Restore closed vaults so account metas still resolve; buy should fail on outcome.
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_y,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_n,
        0,
    ));

    let result =
        svm.process_instruction(&buy_ix(&pdas, true, 0, 1_000, false, u64::MAX), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketAlreadyResolved);
}

#[test]
fn rejects_buy_with_unrelated_mint() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launched_with_buyer(&mut svm, &pdas);
    let ix: Instruction = BuyInstruction {
        buyer: BUYER,
        market: pdas.market,
        pot_mint: POT_MINT,
        buy_mint: OTHER_MINT,
        other_mint: pdas.mint_n,
        pot: pdas.pot,
        user_pot_mint_token_account: pdas.buyer_pot_ata,
        user_buy_mint_token_account: create_keyed_associated_token_account(&BUYER, &OTHER_MINT, 0)
            .address,
        system_program: system_program::ID,
        pot_mint_token_program: SPL_TOKEN_PROGRAM_ID,
        buy_mint_token_program: SPL_TOKEN_PROGRAM_ID,
        associated_token_program: SPL_ASSOCIATED_TOKEN_PROGRAM_ID,
        event_authority: event_authority(),
        program: program_id(),
        in_amount: 0,
        out_amount: 1_000,
        exact_in: false,
        amount_with_slippage: u64::MAX,
    }
    .into();

    let mut accounts = accounts;
    accounts.push(create_keyed_mint_account(
        &OTHER_MINT,
        &Mint {
            mint_authority: Some(AUTHORITY).into(),
            supply: 0,
            decimals: 0,
            is_initialized: true,
            freeze_authority: None.into(),
        },
    ));
    accounts.push(create_keyed_associated_token_account(
        &BUYER,
        &OTHER_MINT,
        0,
    ));

    let result = svm.process_instruction(&ix, &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::InvalidMint);
}

#[test]
fn rejects_buy_when_exact_out_slippage_exceeded() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launched_with_buyer(&mut svm, &pdas);

    let result = svm.process_instruction(&buy_ix(&pdas, true, 0, 10_000, false, 1), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::SlippageExceeded);
}

#[test]
fn rejects_buy_when_exact_in_cannot_mint_any_amount() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launched_with_buyer(&mut svm, &pdas);

    // Cost of one unit at 50/50 with precision 1e6 is ~5e5; budget of 1 → InsufficientLiquidity.
    let result = svm.process_instruction(&buy_ix(&pdas, true, 1, 0, true, 0), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::InsufficientLiquidity);
}

#[test]
fn rejects_buy_with_zero_exact_in_budget() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launched_with_buyer(&mut svm, &pdas);
    let result = svm.process_instruction(&buy_ix(&pdas, true, 0, 0, true, 0), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::InvalidAmount);
}

#[test]
fn rejects_pause_before_launch() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_initialize(&mut svm, &pdas);
    let result = svm.process_instruction(&pause_ix(&pdas, AUTHORITY), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketNotLaunched);
}

#[test]
fn rejects_pause_from_non_authority() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launch(&mut svm, &pdas);
    accounts.push(create_keyed_system_account(&STRANGER, AUTHORITY_LAMPORTS));
    let result = svm.process_instruction(&pause_ix(&pdas, STRANGER), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::UnauthorizedAuthority);
}

#[test]
fn rejects_pause_after_resolve() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launch(&mut svm, &pdas);
    let resolved = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    resolved.assert_success();
    accounts = resolved.accounts;

    let result = svm.process_instruction(&pause_ix(&pdas, AUTHORITY), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketAlreadyResolved);
}

#[test]
fn rejects_resolve_with_undecided_outcome() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launch(&mut svm, &pdas);
    let result = svm.process_instruction(&resolve_ix(&pdas, 2), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketIsAlreadyUndecided);
}

#[test]
fn rejects_resolve_with_invalid_outcome_byte() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launch(&mut svm, &pdas);
    let result = svm.process_instruction(&resolve_ix(&pdas, 4), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::InvalidOutcome);
}

#[test]
fn rejects_resolve_before_launch() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_initialize(&mut svm, &pdas);
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &POT_MINT,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_y,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_n,
        0,
    ));

    let result = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketNotLaunched);
}

#[test]
fn rejects_second_resolve() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launch(&mut svm, &pdas);
    let first = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    first.assert_success();
    accounts = first.accounts;
    // Resolve closes vaults (system-owned). Replace those entries so ATA
    // validation does not see IllegalOwner before MarketAlreadyResolved.
    accounts.retain(|a| a.address != pdas.vault_y && a.address != pdas.vault_n);
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_y,
        0,
    ));
    accounts.push(create_keyed_associated_token_account(
        &pdas.market,
        &pdas.mint_n,
        0,
    ));

    let result = svm.process_instruction(&resolve_ix(&pdas, 1), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketAlreadyResolved);
}

#[test]
fn rejects_claim_before_resolve() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launched_with_buyer(&mut svm, &pdas);
    let bought =
        svm.process_instruction(&buy_ix(&pdas, true, 0, 5_000, false, u64::MAX), &accounts);
    bought.assert_success();
    accounts = bought.accounts;

    let result = svm.process_instruction(&claim_ix(&pdas, true, false), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::MarketNotResolved);
}

#[test]
fn rejects_claim_without_winning_token_account() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launched_with_buyer(&mut svm, &pdas);
    let bought =
        svm.process_instruction(&buy_ix(&pdas, true, 0, 5_000, false, u64::MAX), &accounts);
    bought.assert_success();
    accounts = bought.accounts;

    let resolved = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    resolved.assert_success();
    accounts = resolved.accounts;

    // Outcome Yes but omit the YES token account (program-id sentinel).
    let result = svm.process_instruction(&claim_ix(&pdas, false, false), &accounts);
    expect_custom_error(
        &result,
        FloatingOddsExchangeError::TokenAccountNotInitialized,
    );
}

#[test]
fn rejects_claim_with_zero_winning_balance() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launched_with_buyer(&mut svm, &pdas);

    // Resolve without the buyer holding YES — pass an initialized empty YES ATA.
    accounts.retain(|a| a.address != pdas.buyer_y_ata);
    accounts.push(create_keyed_associated_token_account(
        &BUYER,
        &pdas.mint_y,
        0,
    ));

    let resolved = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    resolved.assert_success();
    accounts = resolved.accounts;

    let result = svm.process_instruction(&claim_ix(&pdas, true, false), &accounts);
    expect_custom_error(&result, FloatingOddsExchangeError::InvalidAmount);
}
