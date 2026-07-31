# anchor-escrow

A minimal SPL-token **escrow** program built with [Anchor](https://www.anchor-lang.com/), with a fast in-process test suite powered by [LiteSVM](https://github.com/LiteSVM/litesvm).

Two parties swap SPL tokens without trusting each other: a **maker** locks token A in a program-controlled vault and states how much token B they want; a **taker** later fills the order, atomically receiving token A while the maker receives token B. If no one fills it, the maker can **refund** and reclaim their tokens.

- **Program ID:** `BuQZzvsCrEbN2TDPs9LovXq95jdX64knK27etKR98s6V`
- Works with both **SPL Token** and **Token-2022** mints (uses `anchor_spl::token_interface`).

## How it works

The escrow account is a **PDA** derived from `["escrow", maker, seed]`. It plays two roles at once:

- **State** — stores the trade terms (`Escrow` struct).
- **Authority** — it owns the `vault` (an ATA of `mint_a`), so only the program, signing with the PDA seeds, can move the locked tokens.

Because the address is deterministic, the taker and refunder can re-derive it without it being stored anywhere, and a fresh `seed` lets one maker run many escrows.

### Instructions

| Instruction                   | Signer | Effect                                                                                                                                     |
| ----------------------------- | ------ | ------------------------------------------------------------------------------------------------------------------------------------------ |
| `make(seed, receive, amount)` | maker  | Creates the escrow PDA, opens the vault, and deposits `amount` of `mint_a`. Records that the maker wants `receive` of `mint_b`.            |
| `take()`                      | taker  | Sends `receive` of `mint_b` to the maker, releases the vaulted `mint_a` to the taker, then closes the vault + escrow (rent back to maker). |
| `refund()`                    | maker  | Returns the vaulted `mint_a` to the maker and closes the vault + escrow.                                                                   |

### `Escrow` state

```rust
pub struct Escrow {
    pub seed: u64,
    pub maker: Pubkey,
    pub mint_a: Pubkey,
    pub mint_b: Pubkey,
    pub receive: u64,
    pub bump: u8,
}
```

### Errors

| Code                            | Meaning                                             |
| ------------------------------- | --------------------------------------------------- |
| `InvalidAmount`                 | `make` called with `amount == 0` or `receive == 0`  |
| `InvalidMaker`                  | escrow's `maker` doesn't match the provided account |
| `InvalidMintA` / `InvalidMintB` | escrow's mints don't match the provided accounts    |

## Project layout

```
programs/anchor-escrow/
├── src/
│   ├── lib.rs              # #[program] entrypoints: make / take / refund
│   ├── state.rs            # Escrow account
│   ├── error.rs            # EscrowError
│   └── instructions/
│       ├── make.rs
│       ├── take.rs
│       └── refund.rs
└── tests/
    └── escrow-litesvm-test.rs   # LiteSVM test suite
idls/
└── anchor_escrow.json     # IDL consumed by declare_program! in tests (see below)
```

## Toolchain

| Tool       | Version       |
| ---------- | ------------- |
| Anchor CLI | 1.1.2         |
| Solana CLI | 3.1.x (Agave) |
| Rust       | 1.89.0        |

## Build

```bash
anchor build
```

This produces:

- `target/deploy/anchor_escrow.so` — the compiled program
- `target/idl/anchor_escrow.json` — the IDL

> **Note:** the `idl-build` feature must enable `anchor-spl` as well, since the instructions use `anchor-spl` token types. This is already set in `programs/anchor-escrow/Cargo.toml`:
>
> ```toml
> idl-build = ["anchor-lang/idl-build", "anchor-spl/idl-build"]
> ```

## Test

```bash
anchor test          # or: cargo test --test escrow-litesvm-test
```

Tests run against the compiled `.so` inside **LiteSVM** (no local validator), using `anchor-litesvm` + `litesvm-utils` helpers. Coverage includes:

- **Happy paths:** make → take swap, make → refund, non-9-decimal mints, multiple escrows per maker.
- **Negative paths:** `make` with zero amount/receive, take with an underfunded taker, refund by a non-maker.
- **Lifecycle:** double-take, refund-after-take (both must fail once the escrow is consumed).

### Testing setup notes

The test file uses `anchor_lang::declare_program!(anchor_escrow)` to generate a typed client from the IDL. A few things this requires (all already configured in this repo):

1. **An `idls/` directory** at the workspace root containing `anchor_escrow.json`. `declare_program!` reads the IDL from there — not from `target/idl/`. After changing any instruction signature or account struct, refresh it:
   ```bash
   anchor build
   cp target/idl/anchor_escrow.json idls/anchor_escrow.json
   ```
2. **`extern crate anchor_lang;`** at the top of the test file — the macro generates `use super::anchor_lang;`, which needs `anchor_lang` bound as a crate-root item.
3. The program `.so` is loaded via `include_bytes!("../../../target/deploy/anchor_escrow.so")` (workspace-level `target/`), so run `anchor build` before `cargo test`.

## Adding tests

Append new cases to `programs/anchor-escrow/tests/escrow-litesvm-test.rs`, reusing the shared helpers (`new_ctx`, `make_escrow`, `setup_taker`, `build_take_ix`, `build_refund_ix`, …) and the single `declare_program!` already in that file.
