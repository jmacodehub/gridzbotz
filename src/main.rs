//! ═══════════════════════════════════════════════════════════════════════════
//! 🚀 PROJECT FLASH V3.8 – Production Grid Trading Bot
//!
//! V3.8 ENHANCEMENTS (fix/grid-init-with-live-price):
//! ✅ initialize_with_price() called between feed warm-up and trading loop
//! ✅ Resolves Active Levels: 0 — grid now always placed before cycle 1
//! ✅ Banner updated to V3.8
//!
//! V3.7 ENHANCEMENTS (Step 5C, Feb 2026):
//! ✅ Single unified tick: process_price_update() owns signal gate,
//!    circuit breaker, crossing detection, fills, metrics, optimizer
//! ✅ Removed duplicate should_reposition() + reposition_grid() from
//!    run_trading_loop -- was causing double repositioning every cycle
//! ✅ Reposition tracking via stats.grid_repositions delta -- accurate
//! ✅ record_failure() now called on tick errors (was missing before)
//! ✅ Cycle status line: Halted / Repositioned / Stable + fills + vol
//!
//! V3.6 ENHANCEMENTS (Step 4 — Session 2, Feb 2026):
//! ✅ --mode paper|live CLI flag overrides bot.execution_mode in TOML
//! ✅ Price feed starts BEFORE engine build — spacing_usd uses real price
//! ✅ Engine builder branches: paper → PaperTradingEngine (fills + spacing)
//!                             live  → honest bail! stub for Step 5
//! ✅ Slippage + fees driven from [execution] config, no hardcoded values
//! ✅ Banner shows active mode, instance name, cluster, spacing
//!
//! V3.5 ENHANCEMENTS - Production-Grade Architecture:
//! ✅ 100% Config-Driven (No Hardcoded Values!)
//! ✅ Flexible Duration Support (hours, minutes, seconds, cycles)
//! ✅ CLI Arguments Support (--config, --duration-minutes, etc.)
//! ✅ Comprehensive Error Handling & Recovery
//! ✅ Graceful Shutdown with Cleanup
//! ✅ Performance Monitoring & Diagnostics
//! ✅ Multi-Environment Support (testing, dev, production)
//!
//! Stage 3 / Step 1 (Feb 2026):
//! ✅ GridBot::new(config) builds engine internally (grid_bot.rs:54)
//! ✅ No longer inject engine from main.rs
//!
//! October 17, 2025 — MASTER V3.5 | February 2026 — V3.8 Grid Init Fix 🔥
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::init;
use solana_grid_bot::config::Config;
use solana_grid_bot::bots::GridBot;
use solana_grid_bot::trading::PriceFeed;

use std::{error::Error, time::Instant, path::PathBuf};
use log::{info, warn, error, debug, trace};
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use anyhow::Result;
use clap::Parser;

// ═══════════════════════════════════════════════════════════════════════════
// COMMAND LINE ARGUMENTS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Parser, Debug)]
#[clap(name = "gridzbotz", version = "3.8.0")]
#[clap(about = "Production-grade Solana grid trading bot", long_about = None)]
struct Args {
    /// Configuration file path
    #[clap(short, long, default_value = "config/master.toml")]
    config: PathBuf,

    /// Execution mode: paper | live
    /// Overrides bot.execution_mode in TOML when provided.
    /// --paper is shorthand for --mode paper.
    #[clap(long, value_name = "MODE")]
    mode: Option<String>,

    /// Shorthand for --mode paper (takes no value)
    #[clap(long, conflicts_with = "mode")]
    paper: bool,

    /// Override test duration in minutes
    #[clap(short = 'd', long)]
    duration_minutes: Option<usize>,

    /// Override test duration in hours
    #[clap(long)]
    duration_hours: Option<usize>,

    /// Override test cycles (expert mode)
    #[clap(long)]
    cycles: Option<usize>,

    /// Enable debug logging
    #[clap(long)]
    debug: bool,

    /// Enable trace logging (very verbose)
    #[clap(long)]
    trace: bool,
}

impl Args {
    /// Resolve the effective execution mode from CLI flags.
    /// Priority: --paper > --mode <value> > TOML bot.execution_mode.
    fn resolved_mode(&self) -> Option<String> {
        if self.paper {
            return Some("paper".to_string());
        }
        self.mode.clone()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SESSION METRICS - Comprehensive Performance Tracking
// ═══════════════════════════════════════════════════════════════════════════

struct SessionMetrics {
    start_time: Instant,
    cycle_times: Vec<u64>,
    repositions: u32,
    errors: u32,
    price_updates: u64,
    successful_cycles: u32,
    failed_cycles: u32,
    failed_price_fetches: u32,
    slow_cycles: u32,
    regime_gate_blocks: u32,
}

impl SessionMetrics {
    fn new() -> Self {
        Self {
            start_time: Instant::now(),
            cycle_times: Vec::with_capacity(1000),
            repositions: 0,
            errors: 0,
            price_updates: 0,
            successful_cycles: 0,
            failed_cycles: 0,
            failed_price_fetches: 0,
            slow_cycles: 0,
            regime_gate_blocks: 0,
        }
    }

    fn record_cycle(&mut self, duration_ms: u64, slow_threshold: u64) {
        self.cycle_times.push(duration_ms);
        self.successful_cycles += 1;
        if duration_ms > slow_threshold {
            self.slow_cycles += 1;
        }
    }

    fn record_failure(&mut self) {
        self.failed_cycles += 1;
        self.errors += 1;
    }

    fn avg_cycle_time(&self) -> f64 {
        if self.cycle_times.is_empty() { 0.0 } else {
            self.cycle_times.iter().sum::<u64>() as f64 / self.cycle_times.len() as f64
        }
    }

    fn min_cycle_time(&self) -> u64 {
        *self.cycle_times.iter().min().unwrap_or(&0)
    }

    fn max_cycle_time(&self) -> u64 {
        *self.cycle_times.iter().max().unwrap_or(&0)
    }

    fn elapsed_secs(&self) -> f64 {
        self.start_time.elapsed().as_secs_f64()
    }

    fn success_rate(&self) -> f64 {
        let total = self.successful_cycles + self.failed_cycles;
        if total == 0 { return 100.0; }
        (self.successful_cycles as f64 / total as f64) * 100.0
    }

    fn cycles_per_second(&self) -> f64 {
        let elapsed = self.elapsed_secs();
        if elapsed == 0.0 { return 0.0; }
        self.successful_cycles as f64 / elapsed
    }

    fn display_summary(&self, total_cycles: u32) {
        let border = "═".repeat(60);

        println!("\n{}", border);
        println!("  📊 SESSION PERFORMANCE SUMMARY");
        println!("{}", border);
        println!("\n⏱️  TIMING:");
        println!("   Runtime:          {:.2}s", self.elapsed_secs());
        println!("   Total Cycles:     {}", total_cycles);
        println!("   Successful:       {} ({:.1}%)",
                 self.successful_cycles, self.success_rate());
        println!("   Failed:           {}", self.failed_cycles);

        println!("\n⚡ CYCLE PERFORMANCE:");
        println!("   Average:          {:.2}ms", self.avg_cycle_time());
        println!("   Min:              {}ms", self.min_cycle_time());
        println!("   Max:              {}ms", self.max_cycle_time());
        println!("   Slow Cycles:      {}", self.slow_cycles);
        println!("   Throughput:       {:.1} cycles/sec", self.cycles_per_second());

        println!("\n🎯 TRADING ACTIVITY:");
        println!("   Grid Repositions: {}", self.repositions);
        println!("   Price Updates:    {}", self.price_updates);
        println!("   Regime Blocks:    {}", self.regime_gate_blocks);

        println!("\n⚠️  ERRORS:");
        println!("   Total Errors:     {}", self.errors);
        println!("   Failed Fetches:   {}", self.failed_price_fetches);

        println!("\n{}\n", border);
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// BANNER & DISPLAY
// ═══════════════════════════════════════════════════════════════════════════

fn print_banner(config: &Config) {
    let border = "═".repeat(75);
    let mode_label = if config.bot.is_live() {
        "🔴 LIVE — real Jupiter swaps on-chain"
    } else {
        "🟡 PAPER — simulation, fills logged to CSV"
    };

    println!("\n{}", border);
    println!("     🚀 GRIDZBOTZ V3.8 — PRODUCTION GRID TRADING BOT");
    println!("     ⚡ Hybrid Feeds • 10Hz Cycles • Signal-Gated Execution");
    println!("{}", border);
    println!("\n   Mode:        {}", mode_label);
    println!("   Instance:    {} | v{} | {}",
             config.bot.instance_name(),
             config.bot.version,
             config.bot.environment.to_uppercase());
    println!("   Cluster:     {} | {}",
             config.network.cluster,
             config.network.rpc_url
                 .get(..42.min(config.network.rpc_url.len()))
                 .unwrap_or(&config.network.rpc_url));
    println!("{}\n", border);
}

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION LOADING WITH CLI OVERRIDES
// ═══════════════════════════════════════════════════════════════════════════

fn load_configuration(args: &Args) -> Result<Config> {
    info!("🔧 Loading configuration from: {}", args.config.display());

    let mut config = Config::from_file(&args.config)?;

    // ── Mode override (--paper or --mode <value>) ──────────────────────────
    if let Some(mode) = args.resolved_mode() {
        let valid = ["paper", "live"];
        if !valid.contains(&mode.as_str()) {
            anyhow::bail!("Invalid --mode '{}'. Must be 'paper' or 'live'", mode);
        }
        info!("⚡ CLI Override: execution_mode = {}", mode);
        config.bot.execution_mode = mode;
    }

    // ── Duration overrides ────────────────────────────────────────────────
    let mut override_count = 0usize;

    if let Some(cycles) = args.cycles {
        info!("🔄 CLI Override: cycles = {}", cycles);
        config.paper_trading.test_cycles = Some(cycles);
        config.paper_trading.test_duration_minutes = None;
        config.paper_trading.test_duration_hours = None;
        override_count += 1;
    } else if let Some(minutes) = args.duration_minutes {
        info!("⏱️  CLI Override: duration = {} minutes", minutes);
        config.paper_trading.test_duration_minutes = Some(minutes);
        config.paper_trading.test_duration_hours = None;
        config.paper_trading.test_cycles = None;
        override_count += 1;
    } else if let Some(hours) = args.duration_hours {
        info!("⏱️  CLI Override: duration = {} hours", hours);
        config.paper_trading.test_duration_hours = Some(hours);
        config.paper_trading.test_duration_minutes = None;
        config.paper_trading.test_cycles = None;
        override_count += 1;
    }

    if override_count == 0 {
        info!("✅ No duration overrides — using config file settings");
    } else {
        info!("✅ Applied {} duration CLI override(s)", override_count);
    }

    config.validate()?;
    Ok(config)
}

// ═══════════════════════════════════════════════════════════════════════════
// COMPONENT INITIALIZATION (Modular & Robust)
// ═══════════════════════════════════════════════════════════════════════════

/// V3.8: Price feed starts first, then GridBot is built synchronously,
/// then initialize_with_price() places the initial grid at the real
/// market price — BEFORE the trading loop starts.
async fn initialize_components(config: &Config) -> Result<(GridBot, PriceFeed)> {
    info!("🔧 Initializing core components...");

    // ── 1. Price Feed — start before GridBot for banner display ────────────
    info!("🚀 Starting V3.5 Hybrid Price Feed (Pyth/Hermes)...");
    let price_history_size = config.trading.volatility_window as usize;
    let feed = PriceFeed::new(price_history_size);

    feed.start().await
        .map_err(|e| anyhow::anyhow!("Failed to start price feed: {:?}", e))?;

    let startup_delay = config.performance.startup_delay_ms;
    info!("⏳ Warming up price feed ({} ms)...", startup_delay);
    sleep(Duration::from_millis(startup_delay)).await;

    let initial_price = feed.latest_price().await;
    if initial_price <= 0.0 {
        anyhow::bail!(
            "Price feed returned invalid price after warm-up: {}. \
             Check Pyth/Hermes connectivity.",
            initial_price
        );
    }

    let mode = feed.get_mode().await;
    info!("💰 Initial SOL/USD: ${:.4}  (feed mode: {:?})", initial_price, mode);

    // ── 2. GridBot (builds PaperTradingEngine internally) ──────────────────
    info!("🤖 Initializing GridBot V4.5...");
    let mut bot = GridBot::new(config.clone())?;
    bot.initialize().await?;
    info!("✅ GridBot built (grid placement deferred until price known)");

    // ── 3. V3.8 FIX: Initialize grid with live price ───────────────────────
    // This is the critical missing step: grid_initialized is set to false
    // in new(), and was never set to true before the trading loop started.
    // initialize_with_price() calls reposition_grid() at the real market
    // price, setting grid_initialized = true and placing all level orders.
    info!("⚙️  Initializing grid with live price data...");
    bot.initialize_with_price(&feed).await
        .context("Failed to initialize bot grid with price feed")?;
    info!("✅ Bot initialization sequence complete — grid ready for trading!");

    Ok((bot, feed))
}

// ═══════════════════════════════════════════════════════════════════════════
// MAIN TRADING LOOP (Production-Grade)
// ═══════════════════════════════════════════════════════════════════════════

async fn run_trading_loop(
    config: &Config,
    bot: &mut GridBot,
    feed: &PriceFeed,
    shutdown: Arc<AtomicBool>,
) -> Result<SessionMetrics> {
    let mut metrics = SessionMetrics::new();

    let total_cycles = config.paper_trading.calculate_cycles(
        config.performance.cycle_interval_ms
    ) as u32;

    let cycle_interval       = config.performance.cycle_interval_ms;
    let stats_interval       = config.metrics.stats_interval as u32;
    let slow_cycle_threshold = cycle_interval * 3;

    info!("🔥 STARTING TRADING LOOP — V4.5 GRID INIT FIX ACTIVE");
    info!("   Total Cycles:     {}", total_cycles);
    info!("   Cycle Interval:   {}ms ({}Hz)", cycle_interval, 1000 / cycle_interval);
    info!("   Duration:         {:.1} minutes",
          config.paper_trading.duration_seconds() as f64 / 60.0);
    info!("   Stats Interval:   Every {} cycles", stats_interval);
    println!();

    let mut last_reposition_count: u64 = 0;

    for cycle in 1..=total_cycles {
        if shutdown.load(Ordering::Relaxed) {
            warn!("🛑 Graceful shutdown at cycle {}/{}", cycle, total_cycles);
            break;
        }

        let cycle_start = Instant::now();

        // ── Price fetch ─────────────────────────────────────────────────────────
        let price = feed.latest_price().await;
        if price <= 0.0 {
            error!("Invalid price at cycle {}: {}", cycle, price);
            metrics.failed_price_fetches += 1;
            metrics.record_failure();
            sleep(Duration::from_millis(cycle_interval)).await;
            continue;
        }
        metrics.price_updates += 1;

        let volatility = feed.volatility().await;

        // ── Single unified tick ─────────────────────────────────────────────────
        let ts = chrono::Utc::now().timestamp();
        match bot.process_price_update(price, ts).await {
            Ok(_) => {
                let stats = bot.get_stats().await;

                let new_repositions = stats.grid_repositions
                    .saturating_sub(last_reposition_count);
                if new_repositions > 0 {
                    metrics.repositions += new_repositions as u32;
                    last_reposition_count = stats.grid_repositions;
                }

                let status = if stats.trading_paused {
                    metrics.regime_gate_blocks += 1;
                    "🚫 Halted"
                } else if new_repositions > 0 {
                    "🔄 Repositioned"
                } else {
                    "✓ Stable"
                };

                println!(
                    "Cycle {:>4}/{:<4} | SOL ${:>9.4} | Vol {:>5.2}% | Fills {:>3} | Repos {:>3} | {}",
                    cycle, total_cycles,
                    price,
                    volatility,
                    stats.successful_trades,
                    stats.grid_repositions,
                    status,
                );

                metrics.record_cycle(
                    cycle_start.elapsed().as_millis() as u64,
                    slow_cycle_threshold,
                );
            }
            Err(e) => {
                metrics.errors += 1;
                metrics.record_failure();
                println!("Cycle {:>4}/{:<4} | SOL ${:>9.4} | ⚠️  Tick error: {}",
                         cycle, total_cycles, price, e);
                error!("[Main] Tick failed at cycle {}: {}", cycle, e);
            }
        }

        if config.metrics.enable_metrics && cycle % stats_interval == 0 {
            info!("📊 Cycle {:>5} | Avg: {:.1}ms | Repos: {} | Blocks: {} | Errors: {}",
                  cycle, metrics.avg_cycle_time(),
                  metrics.repositions, metrics.regime_gate_blocks, metrics.errors);
        }

        let cycle_time = cycle_start.elapsed().as_millis() as u64;
        if cycle_time > slow_cycle_threshold {
            warn!("⏱️  Slow cycle #{}: {}ms (threshold: {}ms)",
                  cycle, cycle_time, slow_cycle_threshold);
        }
        if cycle_time < cycle_interval {
            let sleep_ms = cycle_interval - cycle_time;
            trace!("Sleeping {}ms", sleep_ms);
            sleep(Duration::from_millis(sleep_ms)).await;
        } else {
            debug!("Cycle #{} overran by {}ms", cycle, cycle_time - cycle_interval);
        }
    }

    Ok(metrics)
}

// ═══════════════════════════════════════════════════════════════════════════
// SHUTDOWN & CLEANUP (Graceful)
// ═══════════════════════════════════════════════════════════════════════════

async fn shutdown_components(bot: &mut GridBot, feed: &PriceFeed) -> Result<()> {
    info!("🧹 Cleaning up components...");
    let final_price = feed.latest_price().await;
    bot.display_status(final_price).await;
    bot.display_strategy_performance().await;
    info!("✅ Cleanup complete");
    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// HELPERS
// ═══════════════════════════════════════════════════════════════════════════

fn setup_logging(args: &Args) {
    let log_level = if args.trace {
        log::LevelFilter::Trace
    } else if args.debug {
        log::LevelFilter::Debug
    } else {
        log::LevelFilter::Info
    };

    env_logger::Builder::from_default_env()
        .filter_level(log_level)
        .format_timestamp_millis()
        .init();

    info!("🔊 Logging initialized at {:?} level", log_level);
}

// ═══════════════════════════════════════════════════════════════════════════
// MAIN ENTRY POINT — V3.8 🚀
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    setup_logging(&args);

    init().map_err(|e| anyhow::anyhow!("Core initialization failed: {:?}", e))?;

    let config = load_configuration(&args)?;

    print_banner(&config);
    config.display_summary();

    let (mut bot, feed) = initialize_components(&config).await?;

    let shutdown       = Arc::new(AtomicBool::new(false));
    let shutdown_clone = Arc::clone(&shutdown);

    tokio::spawn(async move {
        match tokio::signal::ctrl_c().await {
            Ok(()) => {
                warn!("🛑 Ctrl+C received — initiating graceful shutdown");
                shutdown_clone.store(true, Ordering::Relaxed);
            }
            Err(e) => error!("Shutdown signal listener failed: {}", e),
        }
    });

    let result = run_trading_loop(&config, &mut bot, &feed, shutdown).await;

    shutdown_components(&mut bot, &feed).await?;

    match result {
        Ok(metrics) => {
            let total_cycles = config.paper_trading.calculate_cycles(
                config.performance.cycle_interval_ms
            ) as u32;
            metrics.display_summary(total_cycles);

            let feed_metrics = feed.get_metrics().await;
            info!("📡 Feed Statistics:");
            info!("   Mode:          {:?}", feed_metrics.mode);
            info!("   Total Updates: {}", feed_metrics.total_updates);
            let total_failures = feed_metrics.http_failures + feed_metrics.ws_failures;
            let success_rate   = if feed_metrics.total_requests > 0 {
                100.0 - (total_failures as f64 / feed_metrics.total_requests as f64) * 100.0
            } else {
                100.0
            };
            info!("   Success Rate:  {:.1}%", success_rate);
            info!("🌙 Session complete | Runtime: {:.2}s | Avg cycle: {:.2}ms",
                  metrics.elapsed_secs(), metrics.avg_cycle_time());
            println!("\n✅ Trading session completed successfully!\n");
        }
        Err(e) => {
            error!("❌ Trading loop failed: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_slippage_decimal_conversion() {
        let bps: u16 = 100;
        let decimal  = bps as f64 / 10_000.0;
        assert!((decimal - 0.01).abs() < 1e-9);

        let bps: u16 = 50;
        let decimal  = bps as f64 / 10_000.0;
        assert!((decimal - 0.005).abs() < 1e-9);
    }

    #[test]
    fn test_spacing_usd_from_price_and_pct() {
        let price   = 200.0_f64;
        let pct     = 0.15_f64;
        let spacing = price * (pct / 100.0);
        assert!((spacing - 0.30).abs() < 1e-9);
    }

    #[test]
    fn test_args_resolved_mode_paper_flag() {
        let args = Args {
            config: PathBuf::from("config/master.toml"),
            mode: None,
            paper: true,
            duration_minutes: None,
            duration_hours: None,
            cycles: None,
            debug: false,
            trace: false,
        };
        assert_eq!(args.resolved_mode(), Some("paper".to_string()));
    }

    #[test]
    fn test_args_resolved_mode_explicit() {
        let args = Args {
            config: PathBuf::from("config/master.toml"),
            mode: Some("live".to_string()),
            paper: false,
            duration_minutes: None,
            duration_hours: None,
            cycles: None,
            debug: false,
            trace: false,
        };
        assert_eq!(args.resolved_mode(), Some("live".to_string()));
    }

    #[test]
    fn test_args_resolved_mode_none() {
        let args = Args {
            config: PathBuf::from("config/master.toml"),
            mode: None,
            paper: false,
            duration_minutes: None,
            duration_hours: None,
            cycles: None,
            debug: false,
            trace: false,
        };
        assert_eq!(args.resolved_mode(), None);
    }

    #[test]
    fn test_session_metrics_success_rate() {
        let mut m = SessionMetrics::new();
        m.record_cycle(50, 200);
        m.record_cycle(60, 200);
        m.record_failure();
        assert!((m.success_rate() - 66.666).abs() < 0.01);
    }

    #[test]
    fn test_session_metrics_avg_cycle_time() {
        let mut m = SessionMetrics::new();
        m.record_cycle(100, 200);
        m.record_cycle(200, 200);
        assert!((m.avg_cycle_time() - 150.0).abs() < 1e-9);
    }
}
