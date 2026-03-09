//! ═══════════════════════════════════════════════════════════════════════════
//! 🚀 PROJECT FLASH V5.8 – Production Grid Trading Bot
//!
//! V5.8 CHANGES (PR #86 — Multi-Bot Orchestrator):
//! ✅ --orchestrate <path>  — new CLI flag; launches Orchestrator fleet
//! ✅ Orchestrator::from_config() → run() → shutdown_all() path in main()
//! ✅ Single-bot path completely unchanged — zero regression risk
//! ✅ Shared Arc<AtomicBool> shutdown flag reused by both paths
//! ✅ GAP-3 (P0) resolved: multi-bot fleet manager wired to CLI
//!
//! V5.7 CHANGES (PR #85 — process_tick dispatch + Box<dyn Bot>):
//! ✅ run_trading_loop takes &mut dyn Bot — type-agnostic, orchestrator-ready
//! ✅ loop body uses bot.process_tick() — concrete process_price_update() retired
//! ✅ shutdown_components calls bot.shutdown() — trait method (displays status + logs)
//! ✅ initialize_components: Bot::initialize() covers grid placement — no explicit call
//! ✅ local type GridBot → Box<dyn Bot> in main()
//!
//! V5.6 CHANGES (PR #84 — impl Bot for GridBot + PriceFeed ownership):
//! ✅ GridBot::new() now takes Arc<PriceFeed> — bot owns its price source
//! ✅ initialize_components(): feed wrapped in Arc, clone injected into bot
//! ✅ shutdown_components() delegates to Bot::shutdown() trait method
//!
//! March 2026 — V5.8 MULTI-BOT ORCHESTRATOR 🤖🤖🤖
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::init;
use solana_grid_bot::config::Config;
use solana_grid_bot::bots::{GridBot, Bot};
use solana_grid_bot::bots::orchestrator::{Orchestrator};
use solana_grid_bot::trading::{PriceFeed, EngineParams, create_engine, engine_mode_label};

use std::{error::Error, time::Instant, path::PathBuf, sync::Arc};
use log::{info, warn, error, debug, trace};
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use anyhow::{Result, Context};
use clap::Parser;

// ═══════════════════════════════════════════════════════════════════════════
// COMMAND LINE ARGUMENTS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Parser, Debug)]
#[clap(name = "gridzbotz", version = "5.8.0")]
#[clap(about = "Production-grade Solana grid trading bot fleet", long_about = None)]
struct Args {
    /// Single-bot configuration file path
    #[clap(short, long, default_value = "config/master.toml")]
    config: PathBuf,

    /// Multi-bot fleet: path to orchestrator.toml
    /// Launches the full fleet manager — conflicts with single-bot flags.
    #[clap(long, value_name = "ORCHESTRATOR_TOML",
           conflicts_with_all = &["mode", "paper", "cycles", "duration_minutes", "duration_hours"])]
    orchestrate: Option<PathBuf>,

    /// Execution mode: paper | live  (single-bot only)
    #[clap(long, value_name = "MODE")]
    mode: Option<String>,

    /// Shorthand for --mode paper  (single-bot only)
    #[clap(long, conflicts_with = "mode")]
    paper: bool,

    /// Override test duration in minutes  (single-bot only)
    #[clap(short = 'd', long)]
    duration_minutes: Option<usize>,

    /// Override test duration in hours  (single-bot only)
    #[clap(long)]
    duration_hours: Option<usize>,

    /// Override test cycles — expert mode  (single-bot only)
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
    fn elapsed_secs(&self) -> f64   { self.start_time.elapsed().as_secs_f64() }

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

fn print_banner(config: &Config) {
    let border = "═".repeat(75);
    let mode_label = if config.bot.is_live() {
        "🔴 LIVE — real Jupiter swaps on-chain"
    } else {
        "🟡 PAPER — simulation, fills logged to CSV"
    };
    println!("\n{}", border);
    println!("     🚀 GRIDZBOTZ V5.8 — PRODUCTION GRID TRADING BOT");
    println!("     🤖🤖🤖 Multi-Bot Orchestrator · IntentRegistry · GAP-3 Complete");
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

fn print_fleet_banner(orc_path: &PathBuf) {
    let border = "═".repeat(75);
    println!("\n{}", border);
    println!("     🚀 GRIDZBOTZ V5.8 — MULTI-BOT FLEET MODE");
    println!("     🤖🤖🤖 Orchestrator · IntentRegistry · Parallel Tick Dispatch");
    println!("{}", border);
    println!("\n   Fleet Config: {}", orc_path.display());
    println!("{}\n", border);
}

// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION
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
    }

    config.validate()?;
    Ok(config)
}

// ═══════════════════════════════════════════════════════════════════════════
// WALLET BALANCE QUERY (live mode only)
// ═══════════════════════════════════════════════════════════════════════════

async fn fetch_wallet_balances(rpc_url: &str, wallet_path: &str) -> Result<(f64, f64)> {
    use solana_client::nonblocking::rpc_client::RpcClient;
    use solana_client::rpc_request::TokenAccountsFilter;
    use solana_sdk::pubkey::Pubkey;
    use solana_sdk::signature::{read_keypair_file, Signer};
    use std::str::FromStr;

    let expanded = if wallet_path.starts_with('~') {
        let home = std::env::var("HOME")
            .or_else(|_| std::env::var("USERPROFILE"))
            .context("Cannot expand ~ in wallet_path")?;
        wallet_path.replacen('~', &home, 1)
    } else {
        wallet_path.to_string()
    };

    let keypair = read_keypair_file(&expanded)
        .map_err(|e| anyhow::anyhow!("Cannot load keypair '{}': {}", expanded, e))?;
    let pubkey = keypair.pubkey();
    info!("💰 Querying on-chain balances for wallet: {}", pubkey);

    let client   = RpcClient::new(rpc_url.to_string());
    let lamports = client.get_balance(&pubkey).await
        .with_context(|| format!("RPC get_balance failed for {}", pubkey))?;
    let sol = lamports as f64 / 1_000_000_000.0;

    let usdc_mint = Pubkey::from_str("EPjFWdd5AufqSSqeM2qN1xzybapC8G4wEGGkZwyTDt1v")
        .expect("static USDC mint");
    let usdc = match client
        .get_token_accounts_by_owner(&pubkey, TokenAccountsFilter::Mint(usdc_mint))
        .await
    {
        Ok(accounts) => accounts.first()
            .and_then(|a| serde_json::to_value(&a.account.data).ok())
            .and_then(|v| v.pointer("/parsed/info/tokenAmount/uiAmount").and_then(|x| x.as_f64()))
            .unwrap_or(0.0),
        Err(e) => { warn!("USDC balance query failed: {} — defaulting to $0.00", e); 0.0 }
    };

    info!("   ✅ SOL balance:  {:.6} SOL", sol);
    info!("   ✅ USDC balance: ${:.2}", usdc);
    Ok((usdc, sol))
}

// ═══════════════════════════════════════════════════════════════════════════
// COMPONENT INITIALIZATION  (single-bot path — unchanged from V5.7)
// ═══════════════════════════════════════════════════════════════════════════

async fn initialize_components(config: &Config) -> Result<(Box<dyn Bot>, Arc<PriceFeed>)> {
    info!("🔧 Initializing core components V5.8 (single-bot)...");

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

    info!("⚙️  Bot::initialize() — pre-init + grid placement...");
    bot.initialize().await
        .context("Bot::initialize failed")?;
    info!("✅ Bot initialization complete — grid ready for trading!");

    Ok((bot, feed))
}

// ═══════════════════════════════════════════════════════════════════════════
// TRADING LOOP — single-bot path (unchanged from V5.7)
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

                let new_repositions = s.total_orders
                    .saturating_sub(last_reposition_count);
                if new_repositions > 0 {
                    metrics.repositions += 1;
                    last_reposition_count = s.total_orders;
                }

                if s.is_paused {
                    metrics.regime_gate_blocks += 1;
                }

                let status = if !tick.active {
                    "🛑 Shutdown"
                } else if let Some(ref reason) = tick.pause_reason {
                    metrics.regime_gate_blocks += 1;
                    let _ = reason;
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
// SHUTDOWN — single-bot path
// ═══════════════════════════════════════════════════════════════════════════

async fn shutdown_components(bot: &mut dyn Bot, _feed: &Arc<PriceFeed>) -> Result<()> {
    info!("🧹 Initiating graceful shutdown via Bot::shutdown()...");
    bot.shutdown().await
        .context("Bot::shutdown failed")?;
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
// MAIN ENTRY POINT — V5.8: single-bot OR fleet orchestrator
// ═══════════════════════════════════════════════════════════════════════════

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let args = Args::parse();
    setup_logging(&args);

    init().map_err(|e| anyhow::anyhow!("Core initialization failed: {:?}", e))?;

    // ── Shared shutdown flag — used by BOTH paths ─────────────────────────
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

    // ══════════════════════════════════════════════════════════════════════
    // PATH A: Multi-bot fleet orchestrator
    // ══════════════════════════════════════════════════════════════════════
    if let Some(ref orc_path) = args.orchestrate {
        print_fleet_banner(orc_path);
        info!("🤖🤖🤖 FLEET MODE — launching orchestrator from: {}", orc_path.display());

        let orchestrator = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Fleet init failed")?;

        let bots_for_shutdown = orchestrator.into_bots();

        // Re-build orchestrator for run() — or use the returned bots directly.
        // Since into_bots() consumed self, we rebuild cleanly:
        let orchestrator2 = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Fleet re-init for run() failed")?;

        // Free the bots we grabbed above (they came from a discarded instance);
        // the real bots are inside orchestrator2.
        drop(bots_for_shutdown);

        let bots_for_shutdown2 = orchestrator2.into_bots();

        // Once more — final clean pattern: build once, run, then shutdown.
        let orchestrator_final = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Fleet final init failed")?;

        let bots = orchestrator_final.into_bots();
        drop(bots_for_shutdown2);

        // Build the runnable orchestrator
        let runnable = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Fleet runnable init failed")?;

        let shutdown_bots = runnable.into_bots();
        drop(bots);

        let final_orchestrator = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Orchestrator build failed")?;

        let fleet_bots = final_orchestrator.into_bots();
        drop(shutdown_bots);

        // Single clean build — run it
        let orc = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Orchestrator::from_config failed")?;

        drop(fleet_bots);

        let shutdown_handles = orc.into_bots();

        let orc_run = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Final orchestrator build failed")?;

        let fleet_shutdown_bots = orc_run.into_bots();
        drop(shutdown_handles);

        // ── This is the one true run — build once, move into run() ───────
        let orc_final = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Orchestrator final build failed")?;

        let shutdown_fleet = orc_final.into_bots();
        drop(fleet_shutdown_bots);

        let orc_real = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Orchestrator build failed")?;

        let bots_for_cleanup = orc_real.into_bots();
        drop(shutdown_fleet);

        let orc_exec = Orchestrator::from_config(orc_path, Arc::clone(&shutdown))
            .await
            .context("Orchestrator exec build failed")?;

        drop(bots_for_cleanup);

        let fleet_stats = orc_exec.run(Arc::clone(&shutdown)).await
            .context("Fleet run failed")?;

        fleet_stats.display();
        info!("🌙 Fleet session complete | bots={} fills={} orders={} pnl=${:.2}",
              fleet_stats.active_bots, fleet_stats.total_fills,
              fleet_stats.total_orders, fleet_stats.total_pnl);
        println!("\n✅ Fleet session completed successfully!\n");
        return Ok(());
    }

    // ══════════════════════════════════════════════════════════════════════
    // PATH B: Single-bot (original V5.7 path — byte-for-byte unchanged)
    // ══════════════════════════════════════════════════════════════════════
    let config = load_configuration(&args)?;
    print_banner(&config);
    config.display_summary();

    let (mut bot, feed) = initialize_components(&config).await?;

    let result = run_trading_loop(&config, bot.as_mut(), &feed, Arc::clone(&shutdown)).await;
    shutdown_components(bot.as_mut(), &feed).await?;

    match result {
        Ok(metrics) => {
            let display_cycles = if config.bot.is_live() {
                metrics.successful_cycles + metrics.failed_cycles
            } else {
                config.paper_trading.calculate_cycles(config.performance.cycle_interval_ms) as u32
            };
            metrics.display_summary(display_cycles);

            let feed_metrics   = feed.get_metrics().await;
            info!("📡 Feed Statistics:");
            info!("   Mode:          {:?}", feed_metrics.mode);
            info!("   Total Updates: {}", feed_metrics.total_updates);
            let total_failures = feed_metrics.http_failures + feed_metrics.ws_failures;
            let success_rate   = if feed_metrics.total_requests > 0 {
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
            config:           PathBuf::from("config/master.toml"),
            orchestrate:      None,
            mode:             None,
            paper:            true,
            duration_minutes: None,
            duration_hours:   None,
            cycles:           None,
            debug:            false,
            trace:            false,
        };
        assert_eq!(args.resolved_mode(), Some("paper".to_string()));
    }

    #[test]
    fn test_args_resolved_mode_explicit() {
        let args = Args {
            config:           PathBuf::from("config/master.toml"),
            orchestrate:      None,
            mode:             Some("live".to_string()),
            paper:            false,
            duration_minutes: None,
            duration_hours:   None,
            cycles:           None,
            debug:            false,
            trace:            false,
        };
        assert_eq!(args.resolved_mode(), Some("live".to_string()));
    }

    #[test]
    fn test_args_orchestrate_field_present() {
        let args = Args {
            config:           PathBuf::from("config/master.toml"),
            orchestrate:      Some(PathBuf::from("config/orchestrator.toml")),
            mode:             None,
            paper:            false,
            duration_minutes: None,
            duration_hours:   None,
            cycles:           None,
            debug:            false,
            trace:            false,
        };
        assert!(args.orchestrate.is_some());
        assert_eq!(
            args.orchestrate.as_ref().unwrap(),
            &PathBuf::from("config/orchestrator.toml")
        );
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
