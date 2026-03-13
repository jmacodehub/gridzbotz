//! =============================================================================
//! SMOKE TEST - End-to-end Jupiter V6 Execution Proof
//!
//! Proves the full gridzbotz stack in sequence:
//!   [1] Keystore load + pubkey check
//!   [2] Live SOL/USD price via Pyth Hermes
//!   [3] Jupiter quote -> VersionedTransaction (api.jup.ag/swap/v1)
//!   [4] Keystore sign (fee-payer identity verified)
//!   [5] Submit to mainnet  <- only with --submit flag
//!
//! SETUP - create a .env file at project root:
//!   cp .env.example .env
//!   # then fill in GRIDZBOTZ_JUPITER_API_KEY (and other REQUIRED vars)
//!
//! USAGE - Dry run (zero risk, reads .env automatically):
//!   cargo run --bin smoke_test -- --keypair ~/.config/solana/id.json
//!
//! Live submit (0.001 SOL -> USDC, ~$0.08):
//!   cargo run --bin smoke_test -- --keypair ... --submit
//!
//! Override RPC inline (ignores .env RPC_URL):
//!   cargo run --bin smoke_test -- --keypair ... --rpc https://... --submit
//!
//! February 2026 | Project Flash V6.0
//! =============================================================================

use anyhow::{Context, Result};
use clap::Parser;
use log::info;
use std::str::FromStr;
use solana_grid_bot::{
    dex::{JupiterClient, SOL_MINT, USDC_MINT},
    security::keystore::{KeystoreConfig, SecureKeystore},
    trading::price_feed::PriceFeed,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::time::{sleep, Duration};

#[derive(Parser, Debug)]
#[clap(name = "smoke_test")]
#[clap(about = "End-to-end Jupiter V6 execution smoke test")]
struct Args {
    /// Path to your MAINNET keypair
    #[clap(short, long, default_value = "~/.config/solana/id.json")]
    keypair: String,

    /// Mainnet RPC URL. Reads GRIDZBOTZ_RPC_URL from .env if not specified.
    #[clap(short, long, env = "GRIDZBOTZ_RPC_URL", default_value = "https://api.mainnet-beta.solana.com")]
    rpc: String,

    /// Jupiter API key. Reads GRIDZBOTZ_JUPITER_API_KEY from .env if not specified.
    /// Get a free key at https://portal.jup.ag
    #[clap(long, env = "GRIDZBOTZ_JUPITER_API_KEY")]
    jup_key: Option<String>,

    /// Send the real swap (0.001 SOL -> USDC). Default: dry run only.
    #[clap(long)]
    submit: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Load .env file from project root (silently ok if missing)
    dotenv::dotenv().ok();

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_millis()
        .init();

    let args = Args::parse();

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║       🧪 GRIDZBOTZ SMOKE TEST — Jupiter V6 Full Stack       ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!(
        "  Mode:     {}",
        if args.submit {
            "🔴 LIVE  — will send 0.001 SOL -> USDC on mainnet"
        } else {
            "🟡 DRY RUN — no transaction sent"
        }
    );
    println!("  Keypair:  {}", args.keypair);
    // Mask the RPC key in output (show first 40 chars only)
    let rpc_display = if args.rpc.len() > 40 {
        format!("{}...[key]", &args.rpc[..40])
    } else {
        args.rpc.clone()
    };
    println!("  RPC:      {}", rpc_display);
    println!("  Jup API:  https://api.jup.ag | auth: {}",
        if args.jup_key.is_some() { "key configured ✅" } else { "❌ NO KEY — set GRIDZBOTZ_JUPITER_API_KEY in .env" }
    );
    println!();

    // -- [1] Keystore --------------------------------------------------------
    print!("  [1/5] Loading keystore.............. ");
    let keystore = SecureKeystore::from_file(KeystoreConfig {
        keypair_path: args.keypair.clone(),
        max_transaction_amount_usdc: Some(1.0),
        max_daily_trades: Some(5),
        max_daily_volume_usdc: Some(5.0),
    })?;
    let user_pubkey = *keystore.pubkey();
    println!("OK  pubkey: {}", user_pubkey);

    // -- [2] Live SOL price --------------------------------------------------
    print!("  [2/5] Fetching live SOL/USD......... ");
    let feed = PriceFeed::new(20);
    feed.start()
        .await
        .map_err(|e| anyhow::anyhow!("Failed to start Pyth price feed: {}", e))?;
    sleep(Duration::from_millis(1500)).await;
    let sol_price = feed.latest_price().await;
    if sol_price <= 0.0 {
        anyhow::bail!("Price feed returned invalid price: {}", sol_price);
    }
    let trade_value_usd = 0.001 * sol_price;
    println!("OK  ${:.4}  (0.001 SOL approx ${:.4})", sol_price, trade_value_usd);
    info!("Pyth price confirmed: ${:.4}", sol_price);

    // -- [3] Jupiter: quote + build tx ---------------------------------------
    print!("  [3/5] Jupiter quote + build tx...... ");

    // Resolve API key: --jup-key flag → GRIDZBOTZ_JUPITER_API_KEY env var
    let api_key = args.jup_key
        .context("Jupiter API key required. Set GRIDZBOTZ_JUPITER_API_KEY in .env or pass --jup-key")?;

    // Parse mints
    let sol_mint = Pubkey::from_str(SOL_MINT)
        .context("Failed to parse SOL_MINT")?;
    let usdc_mint = Pubkey::from_str(USDC_MINT)
        .context("Failed to parse USDC_MINT")?;

    // V4.1 Constructor: direct params, no config struct
    let jupiter = JupiterClient::new(
        args.rpc.clone(),
        user_pubkey,
        sol_mint,
        usdc_mint,
        1000.0, // initial capital for position tracking
        api_key,
    )?
    .with_priority_fee(10_000, "high".to_string());

    let lamports: u64 = 1_000_000; // 0.001 SOL

    // V4.1 API: simple_swap() returns (VersionedTransaction, last_valid_block)
    let (mut vtx, last_valid) = jupiter
        .simple_swap(sol_mint, usdc_mint, lamports)
        .await
        .context("Jupiter simple_swap failed")?;

    // Extract quote details from logs (price impact printed by simple_swap)
    println!("OK  transaction built | valid until block {}", last_valid);

    // -- [4] Sign ------------------------------------------------------------
    print!("  [4/5] Signing transaction........... ");
    keystore
        .sign_versioned_transaction(&mut vtx)
        .context("Signing failed -- fee-payer mismatch or malformed tx from Jupiter")?;
    println!("OK  Fee-payer verified + signature applied");

    // -- Dry run exit --------------------------------------------------------
    if !args.submit {
        println!();
        println!("  ----------------------------------------------------------------");
        println!("  DRY RUN PASSED -- all 4 stages completed successfully");
        println!();
        println!("  [OK] Keystore:  loaded and verified");
        println!("  [OK] Pyth feed: ${:.4} live price", sol_price);
        println!("  [OK] Jupiter:   swap transaction built (SOL->USDC)");
        println!("  [OK] Signing:   fee-payer checked, signature applied");
        println!();
        println!("  --> To fire the real swap, rerun with --submit");
        println!("  ----------------------------------------------------------------");
        println!();
        return Ok(());
    }

    // -- [5] Submit ----------------------------------------------------------
    print!("  [5/5] Submitting to mainnet......... ");
    let rpc_client = RpcClient::new(args.rpc.clone());
    let sig = rpc_client
        .send_and_confirm_transaction(&vtx)
        .await
        .map_err(|e| anyhow::anyhow!("Transaction rejected: {}", e))?;
    println!("OK  CONFIRMED");

    keystore.record_transaction(trade_value_usd).await;
    let (daily_trades, daily_vol) = keystore.get_daily_stats().await;

    println!();
    println!("  ╔═══════════════════════════════════════════════════════════╗");
    println!("  ║  🎉 FIRST REAL GRIDZBOTZ SWAP — MAINNET CONFIRMED! 🎉    ║");
    println!("  ╚═══════════════════════════════════════════════════════════╝");
    println!();
    println!("  Signature:    {}", sig);
    println!("  Explorer:     https://solscan.io/tx/{}", sig);
    println!("  Swapped:      0.001 SOL -> USDC");
    println!();
    println!("  Daily limits: {}/5 trades | ${:.2}/5.00 volume",
        daily_trades, daily_vol);
    println!();

    Ok(())
}
