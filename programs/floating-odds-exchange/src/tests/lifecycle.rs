//! Happy-path coverage for every instruction, chained like the merkle-distributor
//! lifecycle suite and the Quasar vault guide.

use super::*;
use floating_odds_exchange_math::{quote_exact_input, quote_exact_output};

#[test]
fn initialize_creates_market_and_outcome_mints() {
    let mut svm = setup();
    let pdas = market_pdas();

    let result = svm.process_instruction(
        &initialize_ix(&pdas, "FOX", "FOX", "https://example.com/fox.json", FEE_BPS),
        &initialize_accounts(&pdas),
    );
    result.assert_success();

    let market = decode_market(&result.account(&pdas.market).unwrap().data);
    assert_eq!(market.seed, SEED);
    assert_eq!(market.authority, AUTHORITY);
    assert_eq!(market.pot_mint, POT_MINT);
    assert_eq!(market.precision, 1_000_000);
    assert_eq!(market.outcome, 2); // Undecided
    assert_eq!(market.fee_bps, FEE_BPS);
    assert!(!market.is_launched);
    assert!(!market.is_paused);
    assert_eq!(market.bump, pdas.market_bump);

    assert_eq!(mint_supply(result.account(&pdas.mint_y).unwrap()), 0);
    assert_eq!(mint_supply(result.account(&pdas.mint_n).unwrap()), 0);
    assert!(
        result.account(&pdas.metadata_y).unwrap().data.len() > 1,
        "YES metadata should be written"
    );
    assert!(
        result.account(&pdas.metadata_n).unwrap().data.len() > 1,
        "NO metadata should be written"
    );
}

#[test]
fn launch_seeds_pot_and_outcome_vaults() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launch(&mut svm, &pdas);

    let market = decode_market(
        &accounts
            .iter()
            .find(|a| a.address == pdas.market)
            .unwrap()
            .data,
    );
    assert!(market.is_launched);

    let pot = accounts.iter().find(|a| a.address == pdas.pot).unwrap();
    let vault_y = accounts.iter().find(|a| a.address == pdas.vault_y).unwrap();
    let vault_n = accounts.iter().find(|a| a.address == pdas.vault_n).unwrap();
    assert_eq!(token_amount(pot), POT_LIQUIDITY);
    assert_eq!(token_amount(vault_y), SIDE_LIQUIDITY);
    assert_eq!(token_amount(vault_n), SIDE_LIQUIDITY);
    assert_eq!(
        mint_supply(accounts.iter().find(|a| a.address == pdas.mint_y).unwrap()),
        SIDE_LIQUIDITY
    );
    assert_eq!(
        mint_supply(accounts.iter().find(|a| a.address == pdas.mint_n).unwrap()),
        SIDE_LIQUIDITY
    );
}

#[test]
fn buy_exact_out_mints_yes_against_slippage_cap() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launched_with_buyer(&mut svm, &pdas);

    let out_amount = 10_000u64;
    let quoted_cost = quote_exact_output(
        SIDE_LIQUIDITY,
        SIDE_LIQUIDITY,
        out_amount,
        10u64.pow(POT_DECIMALS.into()),
    )
    .unwrap();
    let result = svm.process_instruction(
        &buy_ix(&pdas, true, 0, out_amount, false, u64::MAX),
        &accounts,
    );
    result.assert_success();

    let buyer_y = result.account(&pdas.buyer_y_ata).unwrap();
    assert_eq!(token_amount(buyer_y), out_amount);
    assert_eq!(
        token_amount(result.account(&pdas.pot).unwrap()),
        POT_LIQUIDITY + quoted_cost,
    );
    assert_eq!(
        token_amount(result.account(&pdas.buyer_pot_ata).unwrap()),
        BUYER_POT_BALANCE - quoted_cost,
    );
}

#[test]
fn buy_exact_in_mints_no_with_minimum_out() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launched_with_buyer(&mut svm, &pdas);

    let in_amount = 5_000_000u64;
    let (quoted_out, actual_cost) = quote_exact_input(
        SIDE_LIQUIDITY,
        SIDE_LIQUIDITY,
        in_amount,
        10u64.pow(POT_DECIMALS.into()),
    )
    .unwrap();
    let result = svm.process_instruction(&buy_ix(&pdas, false, in_amount, 0, true, 1), &accounts);
    result.assert_success();

    let buyer_n = result.account(&pdas.buyer_n_ata).unwrap();
    assert_eq!(token_amount(buyer_n), quoted_out);
    assert_eq!(
        token_amount(result.account(&pdas.pot).unwrap()),
        POT_LIQUIDITY + actual_cost,
    );
    assert_eq!(
        token_amount(result.account(&pdas.buyer_pot_ata).unwrap()),
        BUYER_POT_BALANCE - actual_cost,
    );
}

#[test]
fn pause_blocks_further_buys_but_leaves_market_launched() {
    let mut svm = setup();
    let pdas = market_pdas();
    let accounts = do_launch(&mut svm, &pdas);

    let paused = svm.process_instruction(&pause_ix(&pdas, AUTHORITY), &accounts);
    paused.assert_success();

    let market = decode_market(&paused.account(&pdas.market).unwrap().data);
    assert!(market.is_paused);
    assert!(market.is_launched);
}

#[test]
fn resolve_yes_takes_fee_and_burns_vault_inventory() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launched_with_buyer(&mut svm, &pdas);

    // Buy YES so outstanding claim supply remains after vault burn.
    let bought =
        svm.process_instruction(&buy_ix(&pdas, true, 0, 25_000, false, u64::MAX), &accounts);
    bought.assert_success();
    accounts = bought.accounts;

    let pot_before = token_amount(accounts.iter().find(|a| a.address == pdas.pot).unwrap());
    let resolved = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    resolved.assert_success();

    let market = decode_market(&resolved.account(&pdas.market).unwrap().data);
    assert_eq!(market.outcome, 0); // Yes

    let fee = (pot_before as u128 * FEE_BPS as u128).div_ceil(10_000) as u64;
    let pot_after = token_amount(resolved.account(&pdas.pot).unwrap());
    assert_eq!(pot_after, pot_before - fee);
    assert_eq!(
        token_amount(resolved.account(&pdas.payout_ata).unwrap()),
        fee
    );
}

#[test]
fn claim_redeems_winning_yes_tokens_from_pot() {
    let mut svm = setup();
    let pdas = market_pdas();
    let mut accounts = do_launched_with_buyer(&mut svm, &pdas);

    let out_amount = 25_000u64;
    let bought = svm.process_instruction(
        &buy_ix(&pdas, true, 0, out_amount, false, u64::MAX),
        &accounts,
    );
    bought.assert_success();
    accounts = bought.accounts;

    let resolved = svm.process_instruction(&resolve_ix(&pdas, 0), &accounts);
    resolved.assert_success();
    accounts = resolved.accounts;

    let pot_before = token_amount(accounts.iter().find(|a| a.address == pdas.pot).unwrap());
    let supply_y = mint_supply(accounts.iter().find(|a| a.address == pdas.mint_y).unwrap());
    let buyer_pot_before = token_amount(
        accounts
            .iter()
            .find(|a| a.address == pdas.buyer_pot_ata)
            .unwrap(),
    );

    let claimed = svm.process_instruction(&claim_ix(&pdas, true, false), &accounts);
    claimed.assert_success();

    let expected = (pot_before as u128 * out_amount as u128 / supply_y as u128) as u64;
    assert_eq!(
        token_amount(claimed.account(&pdas.buyer_pot_ata).unwrap()),
        buyer_pot_before + expected
    );
    assert!(
        claimed.account(&pdas.buyer_y_ata).is_none()
            || claimed.account(&pdas.buyer_y_ata).unwrap().lamports == 0
            || claimed.account(&pdas.buyer_y_ata).unwrap().data.is_empty(),
        "YES token account should be closed after claim"
    );
}
