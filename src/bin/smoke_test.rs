//! ═══════════════════════════════════════════════════════════════════════════
//! 🧪 SMOKE TEST — End-to-end Jupiter V6 Execution Proof
//!
//! Proves the full gridzbotz stack in sequence:
//!   [1] Keystore load + pubkey check
//!   [2] Live SOL/USD price via Pyth Hermes
//!   [3] Jupiter V6 quote → VersionedTransaction (ALTs preserved)
//!       Uses Cloudflare DoH to bypass system DNS filtering if needed.
//!   [4] Keystore sign (fee-payer identity verified)
//!   [5] Submit to mainnet  ← only with --submit flag
//!
//! USAGE:
//!   # Dry run — zero risk, proves stack works up to signing:
//!   cargo run --bin smoke_test -- --keypair ~/.config/solana/id.json
//!
//!   # Live — sends ONE 0.001 SOL → USDC swap on mainnet (~$0.08):
//!   cargo run --bin smoke_test -- --keypair ~/.config/solana/id.json --submit
//!
//! ⚠️  Jupiter V6 is mainnet-only — devnet has no liquidity pools.
//! ⚠️  Use your MAINNET keypair, not the devnet one.
//!
//! February 2026 | Project Flash V6.0
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{Context, Result};
use clap::Parser;
use log::info;
use solana_grid_bot::{
    security::keystore::{KeystoreConfig, SecureKeystore},
    trading::{
        jupiter_client::{JupiterClient, JupiterConfig, SOL_MINT, USDC_MINT, resolve_via_doh},
        price_feed::PriceFeed,
    },
};
use solana_client::nonblocking::rpc_client::RpcClient;
use tokio::time::{sleep, Duration};

#[derive(Parser, Debug)]
#[clap(name = "smoke_test")]
#[clap(about = "🧪 End-to-end Jupiter V6 execution smoke test")]
struct Args {
    /// Path to your MAINNET keypair (NOT devnet-keypair.json)
    #[clap(short, long, default_value = "~/.config/solana/id.json")]
    keypair: String,

    /// Mainnet RPC URL
    #[clap(short, long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc: String,

    /// Send the real swap (0.001 SOL → USDC). Default: dry run only.
    #[clap(long)]
    submit: bool,
}

#[tokio::main]
async fn main() -> Result<()> {
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
            "🔴 LIVE  — will send 0.001 SOL → USDC swap on mainnet"
        } else {
            "🟡 DRY RUN — no transaction sent"
        }
    );
    println!("  Keypair:  {}", args.keypair);
    println!("  RPC:      {}", &args.rpc[..args.rpc.len().min(60)]);
    println!();

    // ── [1] Keystore ─────────────────────────────────────────────────────────
    print!("  [1/5] Loading keystore.............. ");
    let keystore = SecureKeystore::from_file(KeystoreConfig {
        keypair_path: args.keypair.clone(),
        max_transaction_amount_usdc: Some(1.0),
        max_daily_trades: Some(5),
        max_daily_volume_usdc: Some(5.0),
    })?;
    println!("✅  pubkey: {}", keystore.pubkey());

    // ── [2] Live SOL price ───────────────────────────────────────────────────
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
    println!("✅  ${:.4}  (0.001 SOL ≈ ${:.4})", sol_price, trade_value_usd);
    info!("💰 Pyth price confirmed: ${:.4}", sol_price);

    // ── [3] Jupiter — DoH-first DNS, then quote + VersionedTransaction ──────────
    print!("  [3/5] Jupiter quote + build tx...... ");

    // Try Cloudflare DoH to resolve quote-api.jup.ag.
    // 1.1.1.1 is reached by IP — no system DNS needed — so this works
    // even when your ISP or router is filtering crypto/DeFi domains.
    let jupiter_base = JupiterClient::new(JupiterConfig::default())?
        .with_priority_fee(10_000);

    let jupiter = match resolve_via_doh("quote-api.jup.ag").await {
        Ok(ip) => {
            info!("🌐 DoH resolved quote-api.jup.ag → {} (system DNS bypassed)", ip);
            jupiter_base
                .with_resolved_host("quote-api.jup.ag", ip)
                .context("Failed to apply DNS override to Jupiter client")?
        }
        Err(e) => {
            info!("🌐 DoH unavailable ({}), falling back to system DNS", e);
            jupiter_base
        }
    };

    let lamports: u64 = 1_000_000; // 0.001 SOL
    let user_pubkey = *keystore.pubkey();

    let (mut vtx, last_valid, quote) = jupiter
        .prepare_swap(SOL_MINT, USDC_MINT, lamports, user_pubkey)
        .await
        .context("Jupiter quote failed")?;

    let out_usdc = quote.out_amount.parse::<u64>().unwrap_or(0) as f64 / 1_000_000.0;
    let impact   = quote.price_impact_pct.parse::<f64>().unwrap_or(0.0);
    println!("✅  {:.4} USDC out | impact {:.4}% | valid until block {}",
        out_usdc, impact, last_valid);

    if impact > 1.0 {
        anyhow::bail!("❌ Price impact too high ({:.2}%) — aborting for safety", impact);
    }

    // ── [4] Sign ───────────────────────────────────────────────────────────
    print!("  [4/5] Signing transaction........... ");
    keystore
        .sign_versioned_transaction(&mut vtx)
        .context("Signing failed — fee-payer mismatch or malformed tx from Jupiter")?;
    println!("✅  Fee-payer verified + signature applied");

    // ── Dry run exit ─────────────────────────────────────────────────────────
    if !args.submit {
        println!();
        println!("  ──────────────────────────────────────────────────────────────");
        println!("  🟡 DRY RUN PASSED — all 4 stages completed successfully");
        println!();
        println!("  ✅ Keystore:  loaded and verified");
        println!("  ✅ Pyth feed: ${:.4} live price", sol_price);
        println!("  ✅ Jupiter:   {:.4} USDC quote received", out_usdc);
        println!("  ✅ Signing:   fee-payer checked, signature applied");
        println!();
        println!("  👉 To fire the real swap, rerun with --submit");
        println!("  ──────────────────────────────────────────────────────────────");
        println!();
        return Ok(());
    }

    // ── [5] Submit ───────────────────────────────────────────────────────────
    print!("  [5/5] Submitting to mainnet......... ");
    let rpc_client = RpcClient::new(args.rpc.clone());
    let sig = rpc_client
        .send_and_confirm_transaction(&vtx)
        .await
        .map_err(|e| anyhow::anyhow!("Transaction rejected — check balance and RPC health: {}", e))?;
    println!("✅  CONFIRMED");

    keystore.record_transaction(trade_value_usd).await;
    let (daily_trades, daily_vol) = keystore.get_daily_stats().await;

    println!();
    println!("  ╔═══════════════════════════════════════════════════════════════╗");
    println!("  ║  🎉 FIRST REAL GRIDZBOTZ SWAP — MAINNET CONFIRMED!           ║");
    println!("  ╚═══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Signature:    {}", sig);
    println!("  Explorer:     https://solscan.io/tx/{}", sig);
    println!("  Swapped:      0.001 SOL → {:.4} USDC", out_usdc);
    println!("  Price impact: {:.4}%", impact);
    println!();
    println!("  📊 Daily limits: {}/5 trades | ${:.2}/5.00 volume",
        daily_trades, daily_vol);
    println!();

    Ok(())
}
