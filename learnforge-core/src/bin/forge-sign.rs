//! forge-sign — Phase 14 pack-signing CLI (D-13).
//!
//! Subcommands: `keygen-root`, `issue-cert`, `sign`. Real bodies land in
//! plan 14-03; this shell only declares the CLI surface so downstream plans
//! build against a stable interface.
//!
//! WASM note (14-RESEARCH Pitfall 1): this bin shares the crate's
//! `[dependencies]` table and is compiled by the unscoped
//! `cargo build --target wasm32-unknown-unknown -p learnforge-core` gate.
//! Keep its dependency footprint to `std::fs` + `clap` only — both compile
//! (though `std::fs` is call-time-broken) on wasm32-unknown-unknown.

use clap::{Parser, Subcommand};

/// LearnForge pack-signing tool: root keygen, issuer cert issuance,
/// and pack signing (root → issuer cert → signed pack chain of trust).
#[derive(Parser)]
#[command(name = "forge-sign", version, about)]
struct Cli {
    #[command(subcommand)]
    command: Command,
}

#[derive(Subcommand)]
enum Command {
    /// Generate the root signing keypair (trust anchor).
    KeygenRoot,
    /// Issue an issuer certificate signed by the root key.
    IssueCert,
    /// Sign a pack JSON file with an issuer's signing key.
    Sign,
}

fn main() {
    let cli = Cli::parse();
    let name = match cli.command {
        Command::KeygenRoot => "keygen-root",
        Command::IssueCert => "issue-cert",
        Command::Sign => "sign",
    };
    eprintln!("forge-sign {name}: not implemented (lands in plan 14-03)");
    std::process::exit(1);
}
