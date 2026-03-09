//! ═══════════════════════════════════════════════════════════════════════════
//! 🚀 PROJECT FLASH V5.8 – Production Grid Trading Bot
//!
//! V5.8 CHANGES (PR #86 — Multi-Bot Orchestrator):
//! ✅ --orchestrate <PATH> flag — PATH A fleet mode via Orchestrator V1.0
//! ✅ PATH B (single-bot) is zero-regression — identical to V5.7
//! ✅ fetch_wallet_balances() delegates to trading::wallet_utils
//!    (single source of truth, shared with orchestrator.rs)
//!
//! V5.7 CHANGES (PR #85 — process_tick dispatch + Box<dyn Bot>):
//! ✅ run_trading_loop takes &mut dyn Bot — type-agnostic, orchestrator-ready
//! ✅ loop body uses bot.process_tick() — concrete process_price_update() retired
//! ✅ shutdown_components calls bot.shutdown() — trait method
//! ✅ initialize_components: Bot::initialize() covers grid placement
//! ✅ local type GridBot → Box<dyn Bot> in main()
//!
//! V5.6 CHANGES (PR #84 — impl Bot for GridBot + PriceFeed ownership):
//! ✅ GridBot::new() now takes Arc<PriceFeed>
//! ✅ initialize_components(): feed wrapped in Arc, clone injected into bot
//! ✅ shutdown_components() delegates to Bot::shutdown() trait method
//!
//! V5.5 CHANGES (PR #73 — Engine Factory Wiring):
//! ✅ 60-line match block → 15-line create_engine() call
//! ✅ All engine construction logic lives in src/trading/engine.rs
//! ✅ main.rs only passes EngineParams (wallet balances for live)
//!
//! March 2026 — V5.8 FLEET COMMANDER 🤖
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::init;
use solana_grid_bot::config::Config;
use solana_grid_bot::bots::{GridBot, Bot, Orchestrator, OrchestratorConfig};
use solana_grid_bot::trading::{
    PriceFeed, EngineParams, create_engine, engine_mode_label,
    fetch_wallet_balances_for_orchestrator,
};

use std::{error::Error, time::Instant, path::PathBuf, sync::Arc};
use log::{info, warn, error, debug};
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, Context};
use clap::Parser;

// ═══════════════════════════════════════════════════════════════════════════
// COMMAND LINE ARGUMENTS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Parser, Debug)]
#[clap(name = "gridzbotz", version = "5.8.0")]
#[clap(about = "Production-grade Solana grid trading bot — single-bot or fleet", long_about = None)]
struct Args {
    /// Configuration file path (single-bot mode)
    #[clap(short, long, default_value = "config/master.toml")]
    config: PathBuf,

    /// Fleet mode: path to orchestrator.toml
    /// When set, launches multi-bot Orchestrator V1.0 (PATH A).
    /// When omitted, runs the standard single-bot loop (PATH B).
    #[clap(long, value_name = "ORCH_TOML", conflicts_with_all = &["mode", "paper", "cycles", "duration_minutes", "duration_hours"])]
    orchestrate: Option<PathBuf>,

    /// Execution mode: paper | live  (single-bot PATH B only)
    #[clap(long, value_name = "MODE")]
    mode: Option<String>,

    /// Shorthand for --mode paper  (single-bot PATH B only)
    #[clap(long, conflicts_with = "mode")]
    paper: bool,

    /// Override test duration in minutes  (single-bot PATH B only)
    #[clap(short = 'd', long)]
    duration_minutes: Option<usize>,

    /// Override test duration in hours  (single-bot PATH B only)
    #[clap(long)]
    duration_hours: Option<usize>,

    /// Override test cycles (expert mode, PATH B only)
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
    fn resolved_mode(&self) -> Option<String> {
        if self.paper { return Some("paper".to_string()); }
        self.mode.clone()
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// SESSION METRICS
// ═══════════════════════════════════════════════════════════════════════════

struct SessionMetrics {
    start_time:           Instant,
    cycle_times:          Vec<u64>,
    repositions:          u32,
    errors:               u32,
    price_updates:        u64,
    successful_cycles:    u32,
    failed_cycles:        u32,
    failed_price_fetches: u32,
    slow_cycles:          u32,
    regime_gate_blocks:   u32,
}

impl SessionMetrics {
    fn new() -> Self {
        Self {
            start_time:           Instant::now(),
            cycle_times:          Vec::with_capacity(1000),
            repositions:          0,
            errors:               0,
            price_updates:        0,
            successful_cycles:    0,
            failed_cycles:        0,
            failed_price_fetches: 0,
            slow_cycles:          0,
            regime_gate_blocks:   0,
        }
    }

    fn record_cycle(&mut self, duration_ms: u64, slow_threshold: u64) {
        self.cycle_times.push(duration_ms);
        self.successful_cycles += 1;
        if duration_ms > slow_threshold { self.slow_cycles += 1; }
    }

    fn record_failure(&mut self) {
        self.failed_cycles += 1;
        self.errors += 1;
    }

    fn avg_cycle_time(&self) -> f64 {
        if self.cycle_times.is_empty() { 0.0 }
        else { self.cycle_times.iter().sum::<u64>() as f64 / self.cycle_times.len() as f64 }
    }

    fn min_cycle_time(&self) -> u64 { *self.cycle_times.iter().min().unwrap_or(&0) }
    fn max_cycle_time(&self) -> u64 { *self.cycle_times.iter().max().unwrap_or(&0) }
    fn elapsed_secs(&self)   -> f64 { self.start_time.elapsed().as_secs_f64() }

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
        println!("   Successful:       {} ({:.1}%)", self.successful_cycles, self.success_rate());
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
// BANNER
// ═══════════════════════════════════════════════════════════════════════════

fn print_banner(config: &Config, fleet_mode: bool) {
    let border = "═".repeat(75);
    let mode_label = if fleet_mode {
        "🟠 FLEET — multi-bot Orchestrator V1.0"
    } else if config.bot.is_live() {
        "🔴 LIVE — real Jupiter swaps on-chain"
    } else {
        "🟡 PAPER — simulation, fills logged to CSV"
    };
    println!("\n{}", border);
    println!("     🚀 GRIDZBOTZ V5.8 — PRODUCTION GRID TRADING BOT");
    println!("     🤖 Multi-Bot Orchestrator V1.0 · GAP-3 Complete · Fleet Ready");
    println!("{}", border);
    println!("\n   Mode:     {}", mode_label);
    if !fleet_mode {
        println!("   Instance: {} | v{} | {}",
                 config.bot.instance_name(),
                 config.bot.version,
                 config.bot.environment.to_uppercase());
        println!("   Cluster:  {} | {}",
                 config.network.cluster,
                 config.network.rpc_url
                     .get(..42.min(config.network.rpc_url.len()))
                     .unwrap_or(&config.network.rpc_url));
    }
    println!("{}\n", border);
}

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION  (PATH B only)
// ═══════════════════════════════════════════════════════════════════════════

fn load_configuration(args: &Args) -> Result<Config> {
    info!("🔧 Loading configuration from: {}", args.config.display());
    let mut config = Config::from_file(&args.config)?;

    if let Some(mode) = args.resolved_mode() {
        let valid = ["paper", "live"];
        if !valid.contains(&mode.as_str()) {
            anyhow::bail!("Invalid --mode '{}'. Must be 'paper' or 'live'", mode);
        }
        info!("⚡ CLI Override: execution_mode = {}", mode);
        config.bot.execution_mode = mode;
    }

    if let Some(cycles) = args.cycles {
        info!("🔄 CLI Override: cycles = {}", cycles);
        config.paper_trading.test_cycles = Some(cycles);
        config.paper_trading.test_duration_minutes = None;
        config.paper_trading.test_duration_hours = None;
    } else if let Some(minutes) = args.duration_minutes {
        info!("⏱️  CLI Override: duration = {} minutes", minutes);
        config.paper_trading.test_duration_minutes = Some(minutes);
        config.paper_trading.test_duration_hours = None;
        config.paper_trading.test_cycles = None;
    } else if let Some(hours) = args.duration_hours {
        info!("⏱️  CLI Override: duration = {} hours", hours);
        config.paper_trading.test_duration_hours = Some(hours);
        config.paper_trading.test_duration_minutes = None;
        config.paper_trading.test_cycles = None;
    }

    config.validate()?;
    Ok(config)
}

// ═══════════════════════════════════════════════════════════════════════════
// WALLET BALANCE QUERY  (live mode, PATH B only)
// Delegates to trading::wallet_utils — single source of truth shared with
// orchestrator.rs so the RPC logic never diverges between the two paths.
// ═══════════════════════════════════════════════════════════════════════════

async fn fetch_wallet_balances(rpc_url: &str, wallet_path: &str) -> Result<(f64, f64)> {
    fetch_wallet_balances_for_orchestrator(rpc_url, wallet_path).await
}

// ═══════════════════════════════════════════════════════════════════════════
// COMPONENT INITIALIZATION  (PATH B — single-bot)
// ═══════════════════════════════════════════════════════════════════════════

async fn initialize_components(config: &Config) -> Result<(Box<dyn Bot>, Arc<PriceFeed>)> {
    info!("🔧 Initializing core components V5.8...");

    info!("🚀 Starting V3.5 Hybrid Price Feed (Pyth/Hermes)...");
    let price_history_size = config.trading.volatility_window as usize;
    let feed = Arc::new(PriceFeed::new(price_history_size));
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

    info!("🛠️  Building TradingEngine via factory: {}", engine_mode_label(config));
    let params = if config.bot.is_live() {
        let (usdc, sol) = fetch_wallet_balances(
            &config.network.rpc_url,
            &config.security.wallet_path,
        ).await.context("Failed to query on-chain wallet balances")?;
        EngineParams { live_price: Some(initial_price), wallet_balances: Some((usdc, sol)) }
    } else {
        EngineParams::default()
    };
    let engine = create_engine(config, params).await?;
    info!("✅ TradingEngine constructed via engine factory");

    info!("🤖 Initializing GridBot V5.8 → Box<dyn Bot>...");
    let mut bot: Box<dyn Bot> = Box::new(
        GridBot::new(config.clone(), engine, Arc::clone(&feed))?
    );
    bot.initialize().await.context("Bot::initialize failed")?;
    info!("✅ Bot initialization complete — grid ready for trading!");

    Ok((bot, feed))
}

// ═══════════════════════════════════════════════════════════════════════════
// TRADING LOOP  (PATH B — single-bot, V5.7 unchanged)
// ═══════════════════════════════════════════════════════════════════════════

async fn run_trading_loop(
    config:   &Config,
    bot:      &mut dyn Bot,
    feed:     &Arc<PriceFeed>,
    shutdown: Arc<AtomicBool>,
) -> Result<SessionMetrics> {
    let mut metrics = SessionMetrics::new();

    let total_cycles = if config.bot.is_live() {
        info!("🔴 Live mode: trading indefinitely until Ctrl+C");
        u32::MAX
    } else {
        config.paper_trading.calculate_cycles(config.performance.cycle_interval_ms) as u32
    };

    let cycle_interval       = config.performance.cycle_interval_ms;
    let stats_interval       = config.metrics.stats_interval as u32;
    let slow_cycle_threshold = cycle_interval * 3;

    info!("🔥 STARTING TRADING LOOP — V5.8 Box<dyn Bot> DISPATCH");
    if config.bot.is_live() {
        info!("   Total Cycles:     ∞ (live mode — Ctrl+C to stop)");
    } else {
        info!("   Total Cycles:     {}", total_cycles);
        info!("   Duration:         {:.1} minutes",
              config.paper_trading.duration_seconds() as f64 / 60.0);
    }
    info!("   Cycle Interval:   {}ms ({}Hz)", cycle_interval, 1000 / cycle_interval);
    info!("   Stats Interval:   Every {} cycles", stats_interval);
    println!();

    let mut last_reposition_count: u64 = 0;

    for cycle in 1..=total_cycles {
        if shutdown.load(Ordering::Relaxed) {
            warn!("🛑 Graceful shutdown at cycle {}", cycle);
            break;
        }

        let cycle_start = Instant::now();

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

        match bot.process_tick().await {
            Ok(tick) => {
                if !tick.active {
                    warn!("🛑 Bot signalled shutdown at cycle {} — exiting loop", cycle);
                    break;
                }

                let s = bot.stats();
                let new_repositions = s.total_orders.saturating_sub(last_reposition_count);
                if new_repositions > 0 {
                    metrics.repositions += 1;
                    last_reposition_count = s.total_orders;
                }
                if s.is_paused { metrics.regime_gate_blocks += 1; }

                let status = if let Some(ref _reason) = tick.pause_reason {
                    metrics.regime_gate_blocks += 1;
                    "🚫 Halted"
                } else if tick.orders_placed > 0 {
                    "🔄 Repositioned"
                } else {
                    "✓ Stable"
                };

                if config.bot.is_live() {
                    println!(
                        "Cycle {:>6} | SOL ${:>9.4} | Vol {:>8.4}% | Fills {:>3} | Orders {:>3} | P&L ${:>8.2} | {}",
                        cycle, price, volatility,
                        tick.fills, tick.orders_placed, s.current_pnl, status,
                    );
                } else {
                    println!(
                        "Cycle {:>4}/{:<4} | SOL ${:>9.4} | Vol {:>8.4}% | Fills {:>3} | Orders {:>3} | P&L ${:>8.2} | {}",
                        cycle, total_cycles, price, volatility,
                        tick.fills, tick.orders_placed, s.current_pnl, status,
                    );
                }

                metrics.record_cycle(
                    cycle_start.elapsed().as_millis() as u64,
                    slow_cycle_threshold,
                );
            }
            Err(e) => {
                metrics.errors += 1;
                metrics.record_failure();
                println!("Cycle {:>4} | SOL ${:>9.4} | ⚠️  Tick error: {}", cycle, price, e);
                error!("[Main] Tick failed at cycle {}: {}", cycle, e);
            }
        }

        if config.metrics.enable_metrics && cycle % stats_interval == 0 {
            let s = bot.stats();
            info!("📊 Cycle {:>5} | Avg: {:.1}ms | Orders: {} | Fills: {} | PnL: ${:.2} | Blocks: {} | Errors: {}",
                  cycle, metrics.avg_cycle_time(),
                  s.total_orders, s.total_fills, s.current_pnl,
                  metrics.regime_gate_blocks, metrics.errors);
        }

        let cycle_time = cycle_start.elapsed().as_millis() as u64;
        if cycle_time > slow_cycle_threshold {
            warn!("⏱️  Slow cycle #{}: {}ms (threshold: {}ms)",
                  cycle, cycle_time, slow_cycle_threshold);
        }
        if cycle_time < cycle_interval {
            sleep(Duration::from_millis(cycle_interval - cycle_time)).await;
        } else {
            debug!("Cycle #{} overran by {}ms", cycle, cycle_time - cycle_interval);
        }
    }

    Ok(metrics)
}

// ═══════════════════════════════════════════════════════════════════════════
// SHUTDOWN  (PATH B only)
// ═══════════════════════════════════════════════════════════════════════════

async fn shutdown_components(bot: &mut dyn Bot, _feed: &Arc<PriceFeed>) -> Result<()> {
    info!("🧹 Initiating graceful shutdown via Bot::shutdown()...");
    bot.shutdown().await.context("Bot::shutdown failed")?;
    info!("✅ Shutdown complete");
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
// MAIN ENTRY POINT V5.8 — Dual-Mode Dispatch
//
// PATH A (──orchestrate):  Orchestrator::from_config() → run() — N bots
// PATH B (default):         initialize_components() → run_trading_loop() — 1 bot
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    setup_logging(&args);
    init().map_err(|e| anyhow::anyhow!("Core initialization failed: {:?}", e))?;

    // ── PATH A: Fleet mode (──orchestrate <orch.toml>) ───────────────────────
    if let Some(ref orch_path) = args.orchestrate {
        // Minimal single-bot config for banner (uses --config default)
        // The orchestrator owns its own per-bot configs; we only need this
        // for the version/env metadata line in the banner.
        let banner_config = Config::from_file(&args.config)
            .unwrap_or_else(|_| Config::default_config());
        print_banner(&banner_config, /*fleet_mode=*/ true);

        let shutdown       = Arc::new(AtomicBool::new(false));
        let shutdown_clone = Arc::clone(&shutdown);

        tokio::spawn(async move {
            match tokio::signal::ctrl_c().await {
                Ok(()) => {
                    warn!("🛑 Ctrl+C — fleet shutdown initiated");
                    shutdown_clone.store(true, Ordering::Relaxed);
                }
                Err(e) => error!("Shutdown signal listener failed: {}", e),
            }
        });

        info!("🤖 Launching Orchestrator from: {}", orch_path.display());
        let orchestrator = Orchestrator::from_config(orch_path, shutdown).await
            .context("Orchestrator initialisation failed")?;
        orchestrator.run().await
            .context("Orchestrator run failed")?;

        println!("\n✅ Fleet session complete!\n");
        return Ok(());
    }

    // ── PATH B: Single-bot mode (default, V5.7 unchanged) ──────────────
    let config = load_configuration(&args)?;
    print_banner(&config, /*fleet_mode=*/ false);
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

    let result = run_trading_loop(&config, bot.as_mut(), &feed, shutdown).await;
    shutdown_components(bot.as_mut(), &feed).await?;

    match result {
        Ok(metrics) => {
            let display_cycles = if config.bot.is_live() {
                metrics.successful_cycles + metrics.failed_cycles
            } else {
                config.paper_trading.calculate_cycles(config.performance.cycle_interval_ms) as u32
            };
            metrics.display_summary(display_cycles);

            let feed_metrics = feed.get_metrics().await;
            info!("📡 Feed Statistics:");
            info!("   Mode:          {:?}", feed_metrics.mode);
            info!("   Total Updates: {}", feed_metrics.total_updates);
            let total_failures = feed_metrics.http_failures + feed_metrics.ws_failures;
            let success_rate = if feed_metrics.total_requests > 0 {
                100.0 - (total_failures as f64 / feed_metrics.total_requests as f64) * 100.0
            } else { 100.0 };
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
            orchestrate: None,
            mode: None, paper: true,
            duration_minutes: None, duration_hours: None, cycles: None,
            debug: false, trace: false,
        };
        assert_eq!(args.resolved_mode(), Some("paper".to_string()));
    }

    #[test]
    fn test_args_resolved_mode_explicit() {
        let args = Args {
            config: PathBuf::from("config/master.toml"),
            orchestrate: None,
            mode: Some("live".to_string()), paper: false,
            duration_minutes: None, duration_hours: None, cycles: None,
            debug: false, trace: false,
        };
        assert_eq!(args.resolved_mode(), Some("live".to_string()));
    }

    #[test]
    fn test_args_orchestrate_flag_present() {
        let args = Args {
            config: PathBuf::from("config/master.toml"),
            orchestrate: Some(PathBuf::from("config/orchestrator.toml")),
            mode: None, paper: false,
            duration_minutes: None, duration_hours: None, cycles: None,
            debug: false, trace: false,
        };
        assert!(args.orchestrate.is_some());
        assert_eq!(
            args.orchestrate.unwrap(),
            PathBuf::from("config/orchestrator.toml")
        );
    }

    #[test]
    fn test_args_orchestrate_default_is_none() {
        let args = Args {
            config: PathBuf::from("config/master.toml"),
            orchestrate: None,
            mode: None, paper: false,
            duration_minutes: None, duration_hours: None, cycles: None,
            debug: false, trace: false,
        };
        assert!(args.orchestrate.is_none(), "Default must be PATH B (single-bot)");
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
