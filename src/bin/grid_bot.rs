//! =============================================================================
//! 🤖 GRID BOT — Live Trading Loop v1.0
//!
//! Architecture:
//!   .env → GridEngine (pure price logic) + JupiterClient + PriceFeed → loop
//!
//!   GridEngine is a pure state machine — zero I/O, fully testable.
//!   The main loop wires it to the real world (price feed + Jupiter + RPC).
//!
//! USAGE:
//!   cargo run --bin grid_bot                    # reads .env, runs live
//!   cargo run --bin grid_bot -- --dry-run       # log signals, no swaps
//!
//! Configure via .env (see .env.example):
//!   RPC_URL, WALLET_PATH, JUPITER_API_KEY
//!   GRID_LOWER_PRICE, GRID_UPPER_PRICE, GRID_LEVELS, GRID_ORDER_SIZE_SOL
//!
//! February 2026 | gridzbotz v1.0
//! =============================================================================

use anyhow::{Context, Result};
use clap::Parser;
use log::{info, warn, error};
use solana_grid_bot::{
    dex::{JupiterClient, JupiterConfig, SOL_MINT, USDC_MINT, resolve_via_doh},
    security::keystore::{KeystoreConfig, SecureKeystore},
    trading::price_feed::PriceFeed,
};
use solana_client::nonblocking::rpc_client::RpcClient;
use solana_sdk::pubkey::Pubkey;
use tokio::time::{sleep, Duration, Instant};

// =============================================================================
// GRID ENGINE — Pure price-level state machine
// =============================================================================

/// Calculates N evenly-spaced price levels between `lower` and `upper`.
/// On each `tick(price)`, returns a Buy/Sell/Hold signal based on
/// whether price crossed a grid level since the last tick.
///
/// GridEngine is the single source of truth for order sizing —
/// order_size_sol is embedded in every Buy/Sell signal it emits.
struct GridEngine {
    level_prices: Vec<f64>,
    order_size_sol: f64,
    last_level: Option<usize>,
    trades_signaled: u64,
}

#[derive(Debug)]
enum GridSignal {
    /// Price crossed DOWN through a level — buy SOL (it got cheaper)
    Buy { level: usize, level_price: f64, order_size_sol: f64 },
    /// Price crossed UP through a level — sell SOL (it got more expensive)
    Sell { level: usize, level_price: f64, order_size_sol: f64 },
    Hold,
}

impl GridEngine {
    fn new(lower: f64, upper: f64, num_levels: usize, order_size_sol: f64) -> Self {
        assert!(upper > lower, "upper_price must be > lower_price");
        assert!(num_levels >= 2, "need at least 2 grid levels");

        let step = (upper - lower) / (num_levels - 1) as f64;
        let level_prices: Vec<f64> = (0..num_levels)
            .map(|i| lower + i as f64 * step)
            .collect();

        info!("📐 Grid Engine initialized:");
        info!("   Range:   ${:.2} — ${:.2}", lower, upper);
        info!("   Levels:  {} (${:.2} spacing each)", num_levels, step);
        info!("   Size:    {} SOL per order", order_size_sol);
        for (i, p) in level_prices.iter().enumerate() {
            info!("   Level {:>2}: ${:.4}", i, p);
        }

        Self { level_prices, order_size_sol, last_level: None, trades_signaled: 0 }
    }

    /// Index of the highest level at or below `price`.
    /// Returns None if price is below all levels.
    fn price_to_level(&self, price: f64) -> Option<usize> {
        self.level_prices.iter().rposition(|&l| l <= price)
    }

    /// Feed a new price tick — returns a trading signal.
    /// First tick always returns Hold (used to prime the level tracker).
    fn tick(&mut self, price: f64) -> GridSignal {
        let current_level = self.price_to_level(price);

        let signal = match (self.last_level, current_level) {
            (Some(prev), Some(curr)) if curr < prev => {
                self.trades_signaled += 1;
                GridSignal::Buy {
                    level: curr,
                    level_price: self.level_prices[curr],
                    order_size_sol: self.order_size_sol,
                }
            }
            (Some(prev), Some(curr)) if curr > prev => {
                self.trades_signaled += 1;
                GridSignal::Sell {
                    level: curr,
                    level_price: self.level_prices[curr],
                    order_size_sol: self.order_size_sol,
                }
            }
            _ => GridSignal::Hold,
        };

        self.last_level = current_level;
        signal
    }

    fn trades_signaled(&self) -> u64 { self.trades_signaled }
}

// =============================================================================
// CLI ARGS
// =============================================================================

#[derive(Parser, Debug)]
#[clap(name = "grid_bot", about = "GRIDZBOTZ — Live SOL/USDC grid trading loop")]
struct Args {
    /// Solana RPC endpoint (reads RPC_URL from .env)
    #[clap(long, env = "RPC_URL", default_value = "https://api.mainnet-beta.solana.com")]
    rpc: String,

    /// Keypair JSON path (reads WALLET_PATH from .env)
    #[clap(long, env = "WALLET_PATH", default_value = "~/.config/solana/id.json")]
    keypair: String,

    /// Jupiter API key (reads JUPITER_API_KEY from .env)
    #[clap(long, env = "JUPITER_API_KEY")]
    jup_key: Option<String>,

    /// Grid lower price bound in USD
    #[clap(long, env = "GRID_LOWER_PRICE", default_value = "60.0")]
    lower: f64,

    /// Grid upper price bound in USD
    #[clap(long, env = "GRID_UPPER_PRICE", default_value = "120.0")]
    upper: f64,

    /// Number of grid price levels
    #[clap(long, env = "GRID_LEVELS", default_value = "10")]
    levels: usize,

    /// SOL amount per grid order
    #[clap(long, env = "GRID_ORDER_SIZE_SOL", default_value = "0.01")]
    order_size: f64,

    /// Dry run: log signals but do NOT submit swaps
    #[clap(long)]
    dry_run: bool,
}

// =============================================================================
// SWAP EXECUTION (with 3-attempt retry for transient Whirlpool errors)
// =============================================================================

async fn execute_swap(
    jupiter: &JupiterClient,
    keystore: &SecureKeystore,
    rpc: &RpcClient,
    input_mint: &str,
    output_mint: &str,
    amount: u64,
    user_pubkey: Pubkey,
    label: &str,
) -> Result<String> {
    for attempt in 1u32..=3 {
        match try_swap(jupiter, keystore, rpc, input_mint, output_mint, amount, user_pubkey).await {
            Ok(sig) => return Ok(sig),
            Err(e) => {
                if attempt < 3 {
                    warn!("   ⚠️  {} attempt {}/3 failed: {}. Retrying in 2s...", label, attempt, e);
                    sleep(Duration::from_secs(2)).await;
                } else {
                    error!("   ❌ {} failed after 3 attempts: {}", label, e);
                    return Err(e);
                }
            }
        }
    }
    unreachable!()
}

async fn try_swap(
    jupiter: &JupiterClient,
    keystore: &SecureKeystore,
    rpc: &RpcClient,
    input_mint: &str,
    output_mint: &str,
    amount: u64,
    user_pubkey: Pubkey,
) -> Result<String> {
    let (mut vtx, _last_valid, quote) = jupiter
        .prepare_swap(input_mint, output_mint, amount, user_pubkey)
        .await
        .context("Jupiter quote failed")?;

    let impact = quote.price_impact_pct.parse::<f64>().unwrap_or(0.0);
    let out_amount = quote.out_amount.parse::<u64>().unwrap_or(0);
    info!("   Quote: {} → {} | impact {:.4}%", amount, out_amount, impact);

    if impact > 1.0 {
        anyhow::bail!("Price impact {:.2}% exceeds 1.0% — skipping swap", impact);
    }

    keystore
        .sign_versioned_transaction(&mut vtx)
        .context("Signing failed")?;

    let sig = rpc
        .send_and_confirm_transaction(&vtx)
        .await
        .map_err(|e| anyhow::anyhow!("Transaction rejected: {}", e))?;

    Ok(sig.to_string())
}

// =============================================================================
// MAIN — Setup + Trading Loop
// =============================================================================

#[tokio::main]
async fn main() -> Result<()> {
    dotenv::dotenv().ok();

    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_millis()
        .init();

    let args = Args::parse();

    println!();
    println!("╔══════════════════════════════════════════════════════════════╗");
    println!("║        🤖 GRIDZBOTZ — Live Grid Trading Engine v1.0         ║");
    println!("╚══════════════════════════════════════════════════════════════╝");
    println!();
    println!("  Mode:    {}",
        if args.dry_run { "🟡 DRY RUN — signals logged, no swaps" }
        else            { "🔴 LIVE    — real swaps on mainnet" });
    println!("  Grid:    ${:.2} — ${:.2} | {} levels | {} SOL/order",
        args.lower, args.upper, args.levels, args.order_size);
    let rpc_display = if args.rpc.len() > 42 { format!("{}...", &args.rpc[..42]) } else { args.rpc.clone() };
    println!("  RPC:     {}", rpc_display);
    println!();

    if !args.dry_run {
        warn!("⚠️  LIVE MODE active. Starting in 3s... (Ctrl+C to abort)");
        sleep(Duration::from_secs(3)).await;
    }

    // ── Keystore ──────────────────────────────────────────────────────
    let keystore = SecureKeystore::from_file(KeystoreConfig {
        keypair_path: args.keypair.clone(),
        max_transaction_amount_usdc: Some(500.0),
        max_daily_trades: Some(1000),
        max_daily_volume_usdc: Some(500.0),
    })?;
    let user_pubkey = *keystore.pubkey();
    info!("🔐 Wallet: {}", user_pubkey);

    // ── Price Feed ────────────────────────────────────────────────────
    let feed = PriceFeed::new(20);
    feed.start().await.map_err(|e| anyhow::anyhow!("Price feed failed: {}", e))?;
    sleep(Duration::from_millis(1500)).await;
    let initial_price = feed.latest_price().await;
    if initial_price <= 0.0 {
        anyhow::bail!("Price feed returned invalid initial price — check Pyth/Hermes connectivity");
    }
    info!("📡 Pyth feed live: SOL = ${:.4}", initial_price);

    // ── Jupiter ───────────────────────────────────────────────────────
    let mut jup_config = JupiterConfig::default();
    if let Some(ref key) = args.jup_key {
        jup_config.api_key = Some(key.clone());
    }
    let jupiter_base = JupiterClient::new(jup_config)?.with_priority_fee(10_000);
    let jupiter = match resolve_via_doh("api.jup.ag").await {
        Ok(ip) => {
            info!("🌐 Jupiter DNS: api.jup.ag → {} (DoH)", ip);
            jupiter_base.with_resolved_host("api.jup.ag", ip)
                .context("Failed to apply DoH DNS override")?
        }
        Err(e) => {
            warn!("⚠️  DoH unavailable ({}), falling back to system DNS", e);
            jupiter_base
        }
    };

    // ── RPC ───────────────────────────────────────────────────────────
    let rpc = RpcClient::new(args.rpc.clone());

    // ── Grid Engine ───────────────────────────────────────────────────
    let mut grid = GridEngine::new(args.lower, args.upper, args.levels, args.order_size);
    grid.tick(initial_price); // prime — no signal on first tick
    info!("✅ Grid primed at ${:.4}", initial_price);

    if !args.dry_run {
        info!("💡 BUY orders spend USDC. SELL orders spend SOL. Ensure both are funded.");
    }

    // ── Trading Loop ──────────────────────────────────────────────────
    info!("🚀 Grid loop started — Ctrl+C to stop gracefully");
    println!();

    let mut tick: u64 = 0;
    let mut last_trade = Instant::now() - Duration::from_secs(60);

    loop {
        tokio::select! {
            _ = tokio::signal::ctrl_c() => {
                info!("");
                info!("⛔ Shutting down gracefully...");
                info!("📊 Session summary: {} signals in {} ticks", grid.trades_signaled(), tick);
                break;
            }

            _ = sleep(Duration::from_secs(1)) => {
                tick += 1;

                let price = feed.latest_price().await;
                if price <= 0.0 {
                    warn!("Tick {}: invalid price — skipping", tick);
                    continue;
                }

                // Heartbeat every 30s
                if tick % 30 == 0 {
                    let status = if last_trade.elapsed() >= Duration::from_secs(5) {
                        "READY".to_string()
                    } else {
                        format!("cooldown {}s", Duration::from_secs(5)
                            .saturating_sub(last_trade.elapsed()).as_secs())
                    };
                    info!("💓 Tick {:>5} | SOL ${:.4} | Signals: {:>3} | {}",
                          tick, price, grid.trades_signaled(), status);
                }

                let signal = grid.tick(price);
                let in_cooldown = last_trade.elapsed() < Duration::from_secs(5);

                match signal {
                    // ── BUY: price crossed DOWN → buy SOL with USDC ──
                    GridSignal::Buy { level, level_price, order_size_sol } => {
                        let usdc_amount = (order_size_sol * price * 1_000_000.0) as u64;
                        info!("🟢 BUY  | Level {:>2} (${:.2}) | SOL ${:.4} | {:.4} USDC → SOL",
                              level, level_price, price, order_size_sol * price);

                        if args.dry_run {
                            info!("   [DRY RUN] Would swap {} µUSDC → ~{} SOL",
                                  usdc_amount, order_size_sol);
                        } else if in_cooldown {
                            warn!("   ⏳ Cooldown active — signal noted, swap skipped");
                        } else {
                            match execute_swap(
                                &jupiter, &keystore, &rpc,
                                USDC_MINT, SOL_MINT,
                                usdc_amount, user_pubkey, "BUY"
                            ).await {
                                Ok(sig) => {
                                    info!("   ✅ BUY confirmed | solscan.io/tx/{}", sig);
                                    last_trade = Instant::now();
                                }
                                Err(e) => error!("   ❌ BUY failed: {}", e),
                            }
                        }
                    }

                    // ── SELL: price crossed UP → sell SOL for USDC ───
                    GridSignal::Sell { level, level_price, order_size_sol } => {
                        let lamports = (order_size_sol * 1_000_000_000.0) as u64;
                        info!("🔴 SELL | Level {:>2} (${:.2}) | SOL ${:.4} | {:.4} SOL → USDC",
                              level, level_price, price, order_size_sol);

                        if args.dry_run {
                            info!("   [DRY RUN] Would swap {} lamports SOL → USDC", lamports);
                        } else if in_cooldown {
                            warn!("   ⏳ Cooldown active — signal noted, swap skipped");
                        } else {
                            match execute_swap(
                                &jupiter, &keystore, &rpc,
                                SOL_MINT, USDC_MINT,
                                lamports, user_pubkey, "SELL"
                            ).await {
                                Ok(sig) => {
                                    info!("   ✅ SELL confirmed | solscan.io/tx/{}", sig);
                                    last_trade = Instant::now();
                                }
                                Err(e) => error!("   ❌ SELL failed: {}", e),
                            }
                        }
                    }

                    GridSignal::Hold => {}
                }
            }
        }
    }

    Ok(())
}

// =============================================================================
// TESTS — Pure grid logic, zero I/O
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    fn make_grid() -> GridEngine {
        // 7 levels: $60, $70, $80, $90, $100, $110, $120
        GridEngine::new(60.0, 120.0, 7, 0.01)
    }

    #[test]
    fn test_levels_calculated_correctly() {
        let grid = make_grid();
        let expected = vec![60.0, 70.0, 80.0, 90.0, 100.0, 110.0, 120.0];
        for (a, b) in grid.level_prices.iter().zip(expected.iter()) {
            assert!((a - b).abs() < 0.001, "level mismatch: {} != {}", a, b);
        }
    }

    #[test]
    fn test_first_tick_is_hold() {
        let mut grid = make_grid();
        assert!(matches!(grid.tick(78.0), GridSignal::Hold));
    }

    #[test]
    fn test_buy_on_downward_crossing() {
        let mut grid = make_grid();
        grid.tick(85.0); // prime between $80-$90
        let sig = grid.tick(65.0); // drop below $70 → BUY
        assert!(matches!(sig, GridSignal::Buy { .. }));
    }

    #[test]
    fn test_sell_on_upward_crossing() {
        let mut grid = make_grid();
        grid.tick(65.0); // prime between $60-$70
        let sig = grid.tick(85.0); // rise above $80 → SELL
        assert!(matches!(sig, GridSignal::Sell { .. }));
    }

    #[test]
    fn test_hold_within_same_level() {
        let mut grid = make_grid();
        grid.tick(82.0);
        assert!(matches!(grid.tick(87.0), GridSignal::Hold));
    }

    #[test]
    fn test_price_below_all_levels() {
        let mut grid = make_grid();
        grid.tick(78.0);
        assert!(matches!(grid.tick(55.0), GridSignal::Hold));
    }

    #[test]
    fn test_trade_counter_increments() {
        let mut grid = make_grid();
        grid.tick(78.0);
        grid.tick(65.0); // BUY
        grid.tick(85.0); // SELL
        assert_eq!(grid.trades_signaled(), 2);
    }

    #[test]
    fn test_order_size_in_signal() {
        let mut grid = GridEngine::new(60.0, 120.0, 7, 0.05);
        grid.tick(85.0);
        match grid.tick(65.0) {
            GridSignal::Buy { order_size_sol, .. } => {
                assert!((order_size_sol - 0.05).abs() < 0.0001);
            }
            _ => panic!("expected Buy signal"),
        }
    }
}
