//! =============================================================================
//! SMOKE TEST - End-to-end Jupiter V6 Execution Proof
//!
//! Proves the full gridzbotz stack in sequence:
//!   [1] Keystore load + pubkey check
//!   [2] Live SOL/USD price via Pyth Hermes
//!   [3] Jupiter V6 quote -> VersionedTransaction (ALTs preserved)
//!   [4] Keystore sign (fee-payer identity verified)
//!   [5] Submit to mainnet  <- only with --submit flag
//!
//! USAGE - Dry run (zero risk):
//!   cargo run --bin smoke_test -- --keypair ~/.config/solana/id.json
//!
//! If quote-api.jup.ag is DNS-blocked, look up the IP and pass it:
//!   1. Visit https://dnschecker.org/#A/quote-api.jup.ag in your browser
//!   2. Copy any IP shown (e.g. 104.26.12.35)
//!   3. cargo run --bin smoke_test -- --keypair ... --jup-ip 104.26.12.35
//!
//! Live submit (0.001 SOL -> USDC, ~$0.08):
//!   cargo run --bin smoke_test -- --keypair ... --jup-ip <IP> --submit
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

    /// Mainnet RPC URL
    #[clap(short, long, default_value = "https://api.mainnet-beta.solana.com")]
    rpc: String,

    /// Hardcode the Jupiter API IP to bypass DNS filtering.
    /// Get it from: https://dnschecker.org/#A/quote-api.jup.ag
    /// Example: --jup-ip 104.26.12.35
    #[clap(long)]
    jup_ip: Option<String>,

    /// Send the real swap (0.001 SOL -> USDC). Default: dry run only.
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
            "🔴 LIVE  — will send 0.001 SOL -> USDC on mainnet"
        } else {
            "🟡 DRY RUN — no transaction sent"
        }
    );
    println!("  Keypair:  {}", args.keypair);
    println!("  RPC:      {}", &args.rpc[..args.rpc.len().min(60)]);
    if let Some(ref ip) = args.jup_ip {
        println!("  Jup IP:   {} (DNS bypass)", ip);
    }
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

    let jupiter_base = JupiterClient::new(JupiterConfig::default())?
        .with_priority_fee(10_000);

    // DNS resolution priority:
    //   1. --jup-ip flag (user-supplied, hardcoded bypass)
    //   2. Cloudflare DoH (1.1.1.1) with CNAME following
    //   3. Google DoH (8.8.8.8) with CNAME following
    //   4. System DNS fallback
    let jupiter = if let Some(ref ip_str) = args.jup_ip {
        let ip: IpAddr = ip_str
            .parse()
            .with_context(|| format!("Invalid --jup-ip value: '{}'", ip_str))?;
        info!("[DNS] Using hardcoded IP from --jup-ip: {}", ip);
        jupiter_base
            .with_resolved_host("quote-api.jup.ag", ip)
            .context("Failed to apply --jup-ip DNS override")?;
        jupiter_base
            .with_resolved_host("quote-api.jup.ag", ip)
            .context("Failed to apply --jup-ip DNS override")?
    } else {
        match resolve_via_doh("quote-api.jup.ag").await {
            Ok(ip) => {
                info!("[DNS] DoH resolved quote-api.jup.ag -> {} (system DNS bypassed)", ip);
                jupiter_base
                    .with_resolved_host("quote-api.jup.ag", ip)
                    .context("Failed to apply DoH DNS override")?
            }
            Err(e) => {
                info!("[DNS] DoH failed ({}), falling back to system DNS", e);
                info!("[DNS] Tip: run with --jup-ip <IP> to bypass DNS filtering");
                info!("[DNS] Get IP from: https://dnschecker.org/#A/quote-api.jup.ag");
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
        .map_err(|e| anyhow::anyhow!("Transaction rejected -- check balance and RPC health: {}", e))?;
    println!("OK  CONFIRMED");

    keystore.record_transaction(trade_value_usd).await;
    let (daily_trades, daily_vol) = keystore.get_daily_stats().await;

    println!();
    println!("  ╔═══════════════════════════════════════════════════════════╗");
    println!("  ║  FIRST REAL GRIDZBOTZ SWAP -- MAINNET CONFIRMED!         ║");
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
