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
//!   # then fill in JUPITER_API_KEY and RPC_URL
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
use std::net::IpAddr;
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
#[clap(about = "End-to-end Jupiter V6 execution smoke test")]
struct Args {
    /// Path to your MAINNET keypair
    #[clap(short, long, default_value = "~/.config/solana/id.json")]
    keypair: String,

    /// Mainnet RPC URL. Reads RPC_URL from .env if not specified.
    #[clap(short, long, env = "RPC_URL", default_value = "https://api.mainnet-beta.solana.com")]
    rpc: String,

    /// Jupiter API key (overrides JUPITER_API_KEY env var).
    /// Get a free key at https://portal.jup.ag
    #[clap(long, env = "JUPITER_API_KEY")]
    jup_key: Option<String>,

    /// Hardcode the Jupiter API IP to bypass DNS filtering (optional).
    /// Get it from: https://dnschecker.org/#A/api.jup.ag
    #[clap(long)]
    jup_ip: Option<String>,

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
    println!("  Jup API:  {} | auth: {}",
        JupiterConfig::default().api_url,
        if args.jup_key.is_some() { "key configured" } else { "NO KEY - set JUPITER_API_KEY" }
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
    println!("OK  pubkey: {}", keystore.pubkey());

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

    // -- [3] Jupiter: resolve DNS, quote, build tx ---------------------------
    print!("  [3/5] Jupiter quote + build tx...... ");

    // Build config - API key from --jup-key flag or JUPITER_API_KEY env var
    let mut config = JupiterConfig::default();
    if let Some(ref key) = args.jup_key {
        config.api_key = Some(key.clone());
    }

    let jupiter_base = JupiterClient::new(config)?
        .with_priority_fee(10_000);

    // DNS resolution priority:
    //   1. --jup-ip flag (explicit override)
    //   2. Cloudflare DoH 1.1.1.1 -> Google DoH 8.8.8.8 (CNAME-aware)
    //   3. System DNS fallback
    let jupiter = if let Some(ref ip_str) = args.jup_ip {
        let ip: IpAddr = ip_str.parse()
            .with_context(|| format!("Invalid --jup-ip: '{}'", ip_str))?;
        info!("[DNS] Hardcoded IP from --jup-ip: {}", ip);
        jupiter_base
            .with_resolved_host("api.jup.ag", ip)
            .context("Failed to apply --jup-ip DNS override")?
    } else {
        match resolve_via_doh("api.jup.ag").await {
            Ok(ip) => {
                info!("[DNS] DoH resolved api.jup.ag -> {} (system DNS bypassed)", ip);
                jupiter_base
                    .with_resolved_host("api.jup.ag", ip)
                    .context("Failed to apply DoH DNS override")?
            }
            Err(e) => {
                info!("[DNS] DoH unavailable ({}), using system DNS", e);
                jupiter_base
            }
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
    println!("OK  {:.4} USDC out | impact {:.4}% | valid until block {}",
        out_usdc, impact, last_valid);

    if impact > 1.0 {
        anyhow::bail!("Price impact too high ({:.2}%) -- aborting for safety", impact);
    }

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
        println!("  [OK] Jupiter:   {:.4} USDC quote received", out_usdc);
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
    println!("  Swapped:      0.001 SOL -> {:.4} USDC", out_usdc);
    println!("  Price impact: {:.4}%", impact);
    println!();
    println!("  Daily limits: {}/5 trades | ${:.2}/5.00 volume",
        daily_trades, daily_vol);
    println!();

    Ok(())
}
