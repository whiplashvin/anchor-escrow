#![allow(unexpected_cfgs)]
extern crate anchor_lang;

use anchor_litesvm::{AnchorContext, AnchorLiteSVM, Instruction, Keypair, Pubkey, Signer};
use litesvm_utils::{AssertionHelpers, TestHelpers};
use anchor_lang::system_program;
use spl_associated_token_account::get_associated_token_address;
use spl_token;

// Generate client modules from the program using declare_program!
anchor_lang::declare_program!(anchor_escrow);

#[test]
fn test_escrow_make_and_take() {
    // ============================================================================
    // 1. Initialize AnchorLiteSVM with the escrow program
    // ============================================================================
    let program_id = anchor_escrow::ID;

    let mut ctx = AnchorLiteSVM::build_with_program(
        program_id,
        include_bytes!("../../../target/deploy/anchor_escrow.so"),
    );

    // ============================================================================
    // 2. Create test accounts
    // ============================================================================
    let maker = ctx.svm.create_funded_account(10_000_000_000).unwrap(); // 10 SOL
    let taker = ctx.svm.create_funded_account(10_000_000_000).unwrap(); // 10 SOL

    // ============================================================================
    // 3. Create token mints and funded token accounts
    // ============================================================================
    let mint_a = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let mint_b = ctx.svm.create_token_mint(&taker, 9).unwrap();

    // Maker's account for mint_a (will deposit into escrow)
    let maker_ata_a = ctx.svm
        .create_associated_token_account(&mint_a.pubkey(), &maker)
        .unwrap();
    ctx.svm
        .mint_to(&mint_a.pubkey(), &maker_ata_a, &maker, 1_000_000_000)
        .unwrap(); // 1.0 tokens

    // Taker's account for mint_b (will send to maker)
    let taker_ata_b = ctx.svm
        .create_associated_token_account(&mint_b.pubkey(), &taker)
        .unwrap();
    ctx.svm
        .mint_to(&mint_b.pubkey(), &taker_ata_b, &taker, 500_000_000)
        .unwrap(); // 0.5 tokens 

    // ============================================================================
    // 4. Build and execute "Make" instruction
    // ============================================================================
    let seed: u64 = 42;
    let escrow_pda = ctx.svm.get_pda(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault = get_associated_token_address(&escrow_pda, &mint_a.pubkey());

    let make_ix = ctx.program()
        .accounts(anchor_escrow::client::accounts::Make {
            maker: maker.pubkey(),
            escrow: escrow_pda,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            maker_ata_a,
            vault,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            system_program: system_program::ID,
        })
        .args(anchor_escrow::client::args::Make {
            seed,
            receive: 500_000_000,  // 0.5 tokens
            amount: 1_000_000_000, // 1.0 tokens
        })
        .instruction()
        .unwrap();

    ctx.execute_instruction(make_ix, &[&maker])
        .unwrap()
        .assert_success();

    // Verify escrow was created and tokens were transferred
    assert!(ctx.account_exists(&escrow_pda), "Escrow account should exist");
    ctx.svm.assert_token_balance(&vault, 1_000_000_000);
    ctx.svm.assert_token_balance(&maker_ata_a, 0);

    // ============================================================================
    // 5. Build and execute "Take" instruction
    // ============================================================================
    let taker_ata_a = get_associated_token_address(&taker.pubkey(), &mint_a.pubkey());
    let maker_ata_b = get_associated_token_address(&maker.pubkey(), &mint_b.pubkey());

    let take_ix = ctx.program()
        .accounts(anchor_escrow::client::accounts::Take {
            taker: taker.pubkey(),
            maker: maker.pubkey(),
            escrow: escrow_pda,
            mint_a: mint_a.pubkey(),
            mint_b: mint_b.pubkey(),
            vault,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            system_program: system_program::ID,
        })
        .args(anchor_escrow::client::args::Take {})
        .instruction()
        .unwrap();

    ctx.execute_instruction(take_ix, &[&taker])
        .unwrap()
        .assert_success();

    // ============================================================================
    // 6. Verify final state
    // ============================================================================

    // Verify accounts were closed
    ctx.svm.assert_account_closed(&escrow_pda);
    ctx.svm.assert_account_closed(&vault);

    // Verify token balances after the swap
    ctx.svm.assert_token_balance(&taker_ata_a, 1_000_000_000); // Taker received mint_a tokens
    ctx.svm.assert_token_balance(&taker_ata_b, 0);             // Taker sent all mint_b tokens
    ctx.svm.assert_token_balance(&maker_ata_b, 500_000_000);   // Maker received mint_b tokens
}


const PROGRAM_SO: &[u8] = include_bytes!("../../../target/deploy/anchor_escrow.so");

struct Handles {
    maker: Keypair,
    mint_a: Keypair,
    mint_b: Keypair,
    maker_ata_a: Pubkey,
    escrow_pda: Pubkey,
    vault: Pubkey,
    seed: u64,
}

fn new_ctx() -> AnchorContext {
    AnchorLiteSVM::build_with_program(anchor_escrow::ID, PROGRAM_SO)
}

fn prepare_maker(ctx: &mut AnchorContext, seed: u64, decimals: u8, deposit: u64) -> Handles {
    let program_id = anchor_escrow::ID;
    let maker = ctx.svm.create_funded_account(10_000_000_000).unwrap();

    let mint_a = ctx.svm.create_token_mint(&maker, decimals).unwrap();
    let mint_b = ctx.svm.create_token_mint(&maker, decimals).unwrap();

    let maker_ata_a = ctx
        .svm
        .create_associated_token_account(&mint_a.pubkey(), &maker)
        .unwrap();
    ctx.svm
        .mint_to(&mint_a.pubkey(), &maker_ata_a, &maker, deposit)
        .unwrap();

    let escrow_pda = ctx.svm.get_pda(
        &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
        &program_id,
    );
    let vault = get_associated_token_address(&escrow_pda, &mint_a.pubkey());

    Handles {
        maker,
        mint_a,
        mint_b,
        maker_ata_a,
        escrow_pda,
        vault,
        seed,
    }
}

fn make_ix(ctx: &AnchorContext, h: &Handles, receive: u64, amount: u64) -> Instruction {
    ctx.program()
        .accounts(anchor_escrow::client::accounts::Make {
            maker: h.maker.pubkey(),
            escrow: h.escrow_pda,
            mint_a: h.mint_a.pubkey(),
            mint_b: h.mint_b.pubkey(),
            maker_ata_a: h.maker_ata_a,
            vault: h.vault,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            system_program: system_program::ID,
        })
        .args(anchor_escrow::client::args::Make {
            seed: h.seed,
            receive,
            amount,
        })
        .instruction()
        .unwrap()
}

fn make_escrow(
    ctx: &mut AnchorContext,
    seed: u64,
    decimals: u8,
    deposit: u64,
    receive: u64,
) -> Handles {
    let h = prepare_maker(ctx, seed, decimals, deposit);
    let ix = make_ix(ctx, &h, receive, deposit);
    ctx.execute_instruction(ix, &[&h.maker])
        .unwrap()
        .assert_success();
    h
}

fn setup_taker(ctx: &mut AnchorContext, h: &Handles, mint_b_amount: u64) -> Keypair {
    let taker = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let taker_ata_b = ctx
        .svm
        .create_associated_token_account(&h.mint_b.pubkey(), &taker)
        .unwrap();
    ctx.svm
        .mint_to(&h.mint_b.pubkey(), &taker_ata_b, &h.maker, mint_b_amount)
        .unwrap();
    taker
}

fn build_take_ix(ctx: &AnchorContext, h: &Handles, taker: &Keypair) -> Instruction {
    let taker_ata_a = get_associated_token_address(&taker.pubkey(), &h.mint_a.pubkey());
    let taker_ata_b = get_associated_token_address(&taker.pubkey(), &h.mint_b.pubkey());
    let maker_ata_b = get_associated_token_address(&h.maker.pubkey(), &h.mint_b.pubkey());

    ctx.program()
        .accounts(anchor_escrow::client::accounts::Take {
            taker: taker.pubkey(),
            maker: h.maker.pubkey(),
            escrow: h.escrow_pda,
            mint_a: h.mint_a.pubkey(),
            mint_b: h.mint_b.pubkey(),
            vault: h.vault,
            taker_ata_a,
            taker_ata_b,
            maker_ata_b,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            system_program: system_program::ID,
        })
        .args(anchor_escrow::client::args::Take {})
        .instruction()
        .unwrap()
}

fn build_refund_ix(
    ctx: &AnchorContext,
    h: &Handles,
    maker: &Pubkey,
    maker_ata_a: &Pubkey,
) -> Instruction {
    ctx.program()
        .accounts(anchor_escrow::client::accounts::Refund {
            maker: *maker,
            escrow: h.escrow_pda,
            mint_a: h.mint_a.pubkey(),
            vault: h.vault,
            maker_ata_a: *maker_ata_a,
            associated_token_program: spl_associated_token_account::id(),
            token_program: spl_token::id(),
            system_program: system_program::ID,
        })
        .args(anchor_escrow::client::args::Refund {})
        .instruction()
        .unwrap()
}

// ============================================================================
// Tier 1 — bug-catching tests
// ============================================================================

/// Refund returns the deposited tokens to the maker and closes vault + escrow.
/// Exercises `refund` end-to-end (the wrong-destination bug would fail here).
#[test]
fn test_refund_returns_tokens() {
    let mut ctx = new_ctx();
    let deposit = 1_000_000_000;
    let h = make_escrow(&mut ctx, 1, 9, deposit, 500_000_000);

    // After make: vault holds the deposit, maker_ata_a is empty.
    ctx.svm.assert_token_balance(&h.vault, deposit);
    ctx.svm.assert_token_balance(&h.maker_ata_a, 0);

    let ix = build_refund_ix(&ctx, &h, &h.maker.pubkey(), &h.maker_ata_a);
    ctx.execute_instruction(ix, &[&h.maker])
        .unwrap()
        .assert_success();

    // Tokens are back with the maker; vault + escrow are closed.
    ctx.svm.assert_token_balance(&h.maker_ata_a, deposit);
    ctx.svm.assert_account_closed(&h.vault);
    ctx.svm.assert_account_closed(&h.escrow_pda);
}

/// A `take` where the taker lacks enough mint_b must FAIL and revert.
/// Catches a swallowed `Result` in the take handler: without `?`, the failing
/// transfer would be ignored and the tx would wrongly "succeed".
#[test]
fn test_take_fails_with_insufficient_taker_balance() {
    let mut ctx = new_ctx();
    let deposit = 1_000_000_000;
    let h = make_escrow(&mut ctx, 2, 9, deposit, 500_000_000);

    // Escrow wants 500_000_000 mint_b but the taker only has 100_000_000.
    let taker = setup_taker(&mut ctx, &h, 100_000_000);

    let ix = build_take_ix(&ctx, &h, &taker);
    ctx.execute_instruction(ix, &[&taker])
        .unwrap()
        .assert_failure();

    // Nothing moved: escrow + vault are untouched.
    assert!(ctx.account_exists(&h.escrow_pda));
    ctx.svm.assert_token_balance(&h.vault, deposit);
}

// ============================================================================
// Tier 2 — validation / negative paths
// ============================================================================

/// `make` with amount == 0 fails with `InvalidAmount`.
#[test]
fn test_make_fails_with_zero_amount() {
    let mut ctx = new_ctx();
    let h = prepare_maker(&mut ctx, 3, 9, 1_000_000_000);
    let ix = make_ix(&ctx, &h, 500_000_000, 0);
    ctx.execute_instruction(ix, &[&h.maker])
        .unwrap()
        .assert_anchor_error("InvalidAmount");
}

/// `make` with receive == 0 fails with `InvalidAmount`.
#[test]
fn test_make_fails_with_zero_receive() {
    let mut ctx = new_ctx();
    let h = prepare_maker(&mut ctx, 4, 9, 1_000_000_000);
    let ix = make_ix(&ctx, &h, 0, 1_000_000_000);
    ctx.execute_instruction(ix, &[&h.maker])
        .unwrap()
        .assert_anchor_error("InvalidAmount");
}

/// Someone who is not the maker cannot refund the escrow. Passing the attacker
/// as `maker` makes the PDA seeds mismatch the escrow account, so Anchor rejects
/// it; the escrow stays intact.
#[test]
fn test_refund_by_non_maker_fails() {
    let mut ctx = new_ctx();
    let h = make_escrow(&mut ctx, 5, 9, 1_000_000_000, 500_000_000);

    let attacker = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let attacker_ata_a = ctx
        .svm
        .create_associated_token_account(&h.mint_a.pubkey(), &attacker)
        .unwrap();

    let ix = build_refund_ix(&ctx, &h, &attacker.pubkey(), &attacker_ata_a);
    ctx.execute_instruction(ix, &[&attacker])
        .unwrap()
        .assert_failure();

    assert!(ctx.account_exists(&h.escrow_pda));
}

// ============================================================================
// Tier 3 — lifecycle / state transitions
// ============================================================================

/// The same escrow cannot be taken twice; the second take fails because the
/// escrow and vault were closed by the first.
#[test]
fn test_double_take_fails() {
    let mut ctx = new_ctx();
    let h = make_escrow(&mut ctx, 6, 9, 1_000_000_000, 500_000_000);
    let taker = setup_taker(&mut ctx, &h, 500_000_000);

    let ix1 = build_take_ix(&ctx, &h, &taker);
    ctx.execute_instruction(ix1, &[&taker])
        .unwrap()
        .assert_success();

    let ix2 = build_take_ix(&ctx, &h, &taker);
    ctx.execute_instruction(ix2, &[&taker])
        .unwrap()
        .assert_failure();
}

/// After a successful take, the maker can no longer refund (escrow consumed).
#[test]
fn test_refund_after_take_fails() {
    let mut ctx = new_ctx();
    let h = make_escrow(&mut ctx, 7, 9, 1_000_000_000, 500_000_000);
    let taker = setup_taker(&mut ctx, &h, 500_000_000);

    let take = build_take_ix(&ctx, &h, &taker);
    ctx.execute_instruction(take, &[&taker])
        .unwrap()
        .assert_success();

    let refund = build_refund_ix(&ctx, &h, &h.maker.pubkey(), &h.maker_ata_a);
    ctx.execute_instruction(refund, &[&h.maker])
        .unwrap()
        .assert_failure();
}

/// One maker can open multiple escrows at distinct PDAs using different seeds.
#[test]
fn test_multiple_escrows_per_maker() {
    let mut ctx = new_ctx();
    let program_id = anchor_escrow::ID;

    let maker = ctx.svm.create_funded_account(10_000_000_000).unwrap();
    let mint_a = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let mint_b = ctx.svm.create_token_mint(&maker, 9).unwrap();
    let maker_ata_a = ctx
        .svm
        .create_associated_token_account(&mint_a.pubkey(), &maker)
        .unwrap();
    // Fund 3.0 so we can lock 1.0 twice and keep 1.0.
    ctx.svm
        .mint_to(&mint_a.pubkey(), &maker_ata_a, &maker, 3_000_000_000)
        .unwrap();

    let mut pdas = Vec::new();
    for seed in [100u64, 101u64] {
        let escrow_pda = ctx.svm.get_pda(
            &[b"escrow", maker.pubkey().as_ref(), &seed.to_le_bytes()],
            &program_id,
        );
        let vault = get_associated_token_address(&escrow_pda, &mint_a.pubkey());

        let ix = ctx
            .program()
            .accounts(anchor_escrow::client::accounts::Make {
                maker: maker.pubkey(),
                escrow: escrow_pda,
                mint_a: mint_a.pubkey(),
                mint_b: mint_b.pubkey(),
                maker_ata_a,
                vault,
                associated_token_program: spl_associated_token_account::id(),
                token_program: spl_token::id(),
                system_program: system_program::ID,
            })
            .args(anchor_escrow::client::args::Make {
                seed,
                receive: 500_000_000,
                amount: 1_000_000_000,
            })
            .instruction()
            .unwrap();

        ctx.execute_instruction(ix, &[&maker])
            .unwrap()
            .assert_success();

        assert!(ctx.account_exists(&escrow_pda));
        ctx.svm.assert_token_balance(&vault, 1_000_000_000);
        pdas.push(escrow_pda);
    }

    // Distinct escrows, and the maker deposited 2.0 total (1.0 remains).
    assert_ne!(pdas[0], pdas[1]);
    ctx.svm.assert_token_balance(&maker_ata_a, 1_000_000_000);
}

// ============================================================================
// Tier 4 — variations
// ============================================================================

/// The swap works with mints that use non-9 decimals (proves `transfer_checked`
/// reads decimals from the mint rather than assuming 9). Also a full happy-path
/// take with balance assertions.
#[test]
fn test_make_take_with_six_decimal_mints() {
    let mut ctx = new_ctx();
    let deposit = 1_000_000; // 1.0 token at 6 decimals
    let receive = 500_000; // 0.5 token at 6 decimals
    let h = make_escrow(&mut ctx, 9, 6, deposit, receive);

    let taker = setup_taker(&mut ctx, &h, receive);
    let ix = build_take_ix(&ctx, &h, &taker);
    ctx.execute_instruction(ix, &[&taker])
        .unwrap()
        .assert_success();

    let taker_ata_a = get_associated_token_address(&taker.pubkey(), &h.mint_a.pubkey());
    let maker_ata_b = get_associated_token_address(&h.maker.pubkey(), &h.mint_b.pubkey());
    ctx.svm.assert_token_balance(&taker_ata_a, deposit); // taker received mint_a
    ctx.svm.assert_token_balance(&maker_ata_b, receive); // maker received mint_b
    ctx.svm.assert_account_closed(&h.vault);
    ctx.svm.assert_account_closed(&h.escrow_pda);
}
