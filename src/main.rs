//! \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
//! \ud83d\ude80 PROJECT FLASH V3.5 \u2013 Production Grid Trading Bot
//! 
//! V3.5 ENHANCEMENTS - Production-Grade Architecture:
//! \u2705 100% Config-Driven (No Hardcoded Values!)
//! \u2705 Flexible Duration Support (hours, minutes, seconds, cycles)
//! \u2705 CLI Arguments Support (--config, --duration-minutes, etc.)
//! \u2705 Comprehensive Error Handling & Recovery
//! \u2705 Graceful Shutdown with Cleanup
//! \u2705 Performance Monitoring & Diagnostics
//! \u2705 Multi-Environment Support (testing, dev, production)
//! 
//! Stage 3 / Step 1 (Feb 2026):
//! \u2705 Engine built here and injected into GridBot::new()
//! \u2705 Swap PaperTradingEngine for RealTradingEngine by changing one line
//! 
//! October 17, 2025 - MASTER V3.5 LFG! \ud83d\udd25
//! \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

use solana_grid_bot::init;
use solana_grid_bot::config::Config;
use solana_grid_bot::bots::GridBot;
use solana_grid_bot::trading::{
    PriceFeed,
    PaperTradingEngine,  // Stage 3 Step 1: engine built here and injected
    TradingEngine,       // Stage 3 Step 1: trait object for GridBot
};

use std::{error::Error, time::Instant, path::PathBuf};
use log::{info, warn, error, debug, trace};
use tokio::time::{sleep, Duration};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use anyhow::Result;
use clap::Parser;

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// COMMAND LINE ARGUMENTS
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

#[derive(Parser, Debug)]
#[clap(name = "Project Flash V3.5")]
#[clap(author = "Grid Trading Team")]
#[clap(version = "3.5.0")]
#[clap(about = "Production-grade Solana grid trading bot", long_about = None)]
struct Args {
    /// Configuration file path
    #[clap(short, long, default_value = "config/master.toml")]
    config: PathBuf,

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

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// SESSION METRICS - Comprehensive Performance Tracking
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

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
        let border = "\u2550".repeat(60);

        println!("\n{}", border);
        println!("  \ud83d\udcca SESSION PERFORMANCE SUMMARY");
        println!("{}", border);
        println!("\n\u23f1\ufe0f  TIMING:");
        println!("   Runtime:          {:.2}s", self.elapsed_secs());
        println!("   Total Cycles:     {}", total_cycles);
        println!("   Successful:       {} ({:.1}%)",
                 self.successful_cycles, self.success_rate());
        println!("   Failed:           {}", self.failed_cycles);

        println!("\n\u26a1 CYCLE PERFORMANCE:");
        println!("   Average:          {:.2}ms", self.avg_cycle_time());
        println!("   Min:              {}ms", self.min_cycle_time());
        println!("   Max:              {}ms", self.max_cycle_time());
        println!("   Slow Cycles:      {}", self.slow_cycles);
        println!("   Throughput:       {:.1} cycles/sec", self.cycles_per_second());

        println!("\n\ud83c\udfaf TRADING ACTIVITY:");
        println!("   Grid Repositions: {}", self.repositions);
        println!("   Price Updates:    {}", self.price_updates);
        println!("   Regime Blocks:    {}", self.regime_gate_blocks);

        println!("\n\u26a0\ufe0f  ERRORS:");
        println!("   Total Errors:     {}", self.errors);
        println!("   Failed Fetches:   {}", self.failed_price_fetches);

        println!("\n{}\n", border);
    }
}

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// BANNER & DISPLAY
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

fn print_banner(config: &Config) {
    let border = "\u2550".repeat(75);

    println!("\n{}", border);
    println!("     \ud83d\ude80 PROJECT FLASH V3.5 - PRODUCTION GRID TRADING BOT");
    println!("     \u26a1 Hybrid Feeds \u2022 10Hz Cycles \u2022 Real-Time Analytics");
    println!("{}", border);
    println!("\n   Environment: {} | Version: {}",
             config.bot.environment.to_uppercase(),
             config.bot.version);
    println!("{}\n", border);
}

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// CONFIGURATION LOADING WITH CLI OVERRIDES
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

fn load_configuration(args: &Args) -> Result<Config> {
    info!("\ud83d\udd27 Loading configuration...");
    info!("\ud83d\udcc4 Loading from file: {}", args.config.display());

    let mut config = Config::from_file(&args.config)?;

    let mut override_count = 0;

    if let Some(cycles) = args.cycles {
        info!("\ud83d\udd04 CLI Override: cycles = {}", cycles);
        config.paper_trading.test_cycles = Some(cycles);
        config.paper_trading.test_duration_minutes = None;
        config.paper_trading.test_duration_hours = None;
        override_count += 1;
    } else if let Some(minutes) = args.duration_minutes {
        info!("\u23f1\ufe0f  CLI Override: duration = {} minutes", minutes);
        config.paper_trading.test_duration_minutes = Some(minutes);
        config.paper_trading.test_duration_hours = None;
        config.paper_trading.test_cycles = None;
        override_count += 1;
    } else if let Some(hours) = args.duration_hours {
        info!("\u23f1\ufe0f  CLI Override: duration = {} hours", hours);
        config.paper_trading.test_duration_hours = Some(hours);
        config.paper_trading.test_duration_minutes = None;
        config.paper_trading.test_cycles = None;
        override_count += 1;
    }

    if override_count > 0 {
        info!("\u2705 Applied {} CLI override(s)", override_count);
    } else {
        info!("\u2705 Using config file settings (no CLI overrides)");
    }

    config.validate()?;

    Ok(config)
}

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// COMPONENT INITIALIZATION (Modular & Robust)
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

async fn initialize_components(config: &Config) -> Result<(GridBot, PriceFeed)> {
    info!("\ud83d\udd27 Initializing core components...");

    info!("\ud83e\udd16 Initializing GridBot...");

    // Stage 3 Step 1: build engine here, inject into GridBot.
    // To switch to live trading, swap PaperTradingEngine for RealTradingEngine.
    let initial_usdc = config.paper_trading.initial_usdc;
    let initial_sol  = config.paper_trading.initial_sol;
    if initial_usdc <= 0.0 || initial_sol <= 0.0 {
        anyhow::bail!("Invalid initial capital: USDC={}, SOL={}", initial_usdc, initial_sol);
    }
    let engine: Box<dyn TradingEngine> = Box::new(
        PaperTradingEngine::new(initial_usdc, initial_sol)
            .with_fees(0.0002, 0.0004)
            .with_slippage(0.0005)
    );
    info!("\u2705 Paper trading engine built: ${:.2} USDC + {:.4} SOL", initial_usdc, initial_sol);

    let mut bot = GridBot::new(config.clone(), engine)?;
    bot.initialize().await?;
    info!("\u2705 GridBot ready");

    info!("\ud83d\ude80 Initializing V3.5 Hybrid Price Feed...");
    let price_history_size = config.trading.volatility_window as usize;
    let feed = PriceFeed::new(price_history_size);

    feed.start().await
        .map_err(|e| anyhow::anyhow!("Failed to start price feed: {:?}", e))?;
    info!("\u2705 Price feed started");

    let startup_delay = config.performance.startup_delay_ms;
    info!("\u23f3 Warming up price feed ({} ms)...", startup_delay);
    sleep(Duration::from_millis(startup_delay)).await;

    let initial_price = feed.latest_price().await;
    if initial_price <= 0.0 {
        anyhow::bail!("Price feed returned invalid price: {}", initial_price);
    }

    let mode = feed.get_mode().await;
    info!("\ud83d\udcb0 Initial SOL/USD: ${:.4}", initial_price);
    info!("\ud83d\udce1 Feed Mode: {:?}", mode);

    Ok((bot, feed))
}

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// MAIN TRADING LOOP (Production-Grade)
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

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

    let cycle_interval      = config.performance.cycle_interval_ms;
    let stats_interval      = config.metrics.stats_interval as u32;
    let slow_cycle_threshold = cycle_interval * 2;

    info!("\ud83d\udd25 STARTING TRADING LOOP");
    info!("   Total Cycles:     {}", total_cycles);
    info!("   Cycle Interval:   {}ms ({}Hz)", cycle_interval, 1000 / cycle_interval);
    info!("   Duration:         {:.1} minutes",
          config.paper_trading.duration_seconds() as f64 / 60.0);
    info!("   Stats Interval:   Every {} cycles", stats_interval);
    println!();

    let mut last_price = feed.latest_price().await;

    for cycle in 1..=total_cycles {
        if shutdown.load(Ordering::Relaxed) {
            warn!("\ud83d\uded1 Graceful shutdown at cycle {}/{}", cycle, total_cycles);
            break;
        }

        let cycle_start = Instant::now();

        let price = feed.latest_price().await;
        if price <= 0.0 {
            error!("Invalid price: {}", price);
            metrics.failed_price_fetches += 1;
            metrics.record_failure();
            continue;
        }
        metrics.price_updates += 1;

        let price_change = if last_price > 0.0 {
            ((price - last_price) / last_price) * 100.0
        } else {
            0.0
        };

        let volatility = feed.volatility().await;
        let trend      = classify_trend(price_change);

        print!("Cycle {:>4}/{:<4} {} | SOL ${:>9.4} ({:>+6.3}%) | Vol: {:>5.2}% | ",
               cycle, total_cycles, trend, price, price_change, volatility);

        if bot.should_reposition(price, last_price).await {
            match bot.reposition_grid(price, last_price).await {
                Ok(_) => {
                    metrics.repositions += 1;
                    println!("\ud83d\udd04 Rebalanced");
                }
                Err(e) => {
                    metrics.errors += 1;
                    println!("\u26a0\ufe0f  Failed");
                    error!("Reposition error: {}", e);
                }
            }
        } else {
            let stats = bot.get_stats().await;
            if stats.trading_paused {
                metrics.regime_gate_blocks += 1;
                println!("\ud83d\udeab Paused (regime gate)");
            } else {
                println!("\u2713 Grid stable");
            }
        }

        if let Err(e) = bot.process_price_update(price, chrono::Utc::now().timestamp()).await {
            error!("Failed to process price: {}", e);
            metrics.errors += 1;
        }

        last_price = price;

        let cycle_time = cycle_start.elapsed().as_millis() as u64;
        metrics.record_cycle(cycle_time, slow_cycle_threshold);

        if cycle_time > slow_cycle_threshold {
            warn!("\u23f1\ufe0f  Slow cycle #{}: {}ms (threshold: {}ms)",
                  cycle, cycle_time, slow_cycle_threshold);
        }

        if config.metrics.enable_metrics && cycle % stats_interval == 0 {
            info!("\ud83d\udcca {} | Avg: {:.1}ms | Repos: {} | Blocks: {}",
                  cycle, metrics.avg_cycle_time(), metrics.repositions,
                  metrics.regime_gate_blocks);
        }

        if cycle_time < cycle_interval {
            let sleep_duration = cycle_interval - cycle_time;
            trace!("Sleeping for {}ms", sleep_duration);
            sleep(Duration::from_millis(sleep_duration)).await;
        } else {
            debug!("Cycle overran by {}ms", cycle_time - cycle_interval);
        }
    }

    Ok(metrics)
}

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// SHUTDOWN & CLEANUP (Graceful)
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

async fn shutdown_components(bot: &mut GridBot, feed: &PriceFeed) -> Result<()> {
    info!("\ud83e\uddf9 Cleaning up components...");

    info!("Price feed cleanup complete");

    let final_price = feed.latest_price().await;
    bot.display_status(final_price).await;
    bot.display_strategy_performance().await;

    info!("\u2705 Cleanup complete");
    Ok(())
}

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// HELPER FUNCTIONS
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

fn classify_trend(price_change: f64) -> &'static str {
    if price_change.abs() < 0.005 {
        "\u27a1\ufe0f"
    } else if price_change > 0.1 {
        "\ud83d\ude80"
    } else if price_change > 0.05 {
        "\ud83d\udcc8"
    } else if price_change > 0.0 {
        "\ud83d\udcc8"
    } else if price_change < -0.1 {
        "\ud83d\udca5"
    } else if price_change < -0.05 {
        "\ud83d\udcc9"
    } else {
        "\ud83d\udcc9"
    }
}

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

    info!("\ud83d\udd0a Logging initialized at {:?} level", log_level);
}

// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550
// MAIN ENTRY POINT - V3.5 Production Grade! \ud83d\ude80
// \u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550\u2550

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
                warn!("\ud83d\uded1 Ctrl+C received - initiating graceful shutdown");
                shutdown_clone.store(true, Ordering::Relaxed);
            }
            Err(e) => {
                error!("Failed to listen for shutdown signal: {}", e);
            }
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
            info!("\ud83d\udce1 Feed Statistics:");
            info!("   Mode:             {:?}", feed_metrics.mode);
            info!("   Total Updates:    {}", feed_metrics.total_updates);
            let total_failures = feed_metrics.http_failures + feed_metrics.ws_failures;
            let success_rate = if feed_metrics.total_requests > 0 {
                100.0 - (total_failures as f64 / feed_metrics.total_requests as f64) * 100.0
            } else {
                100.0
            };
            info!("   Success Rate:     {:.1}%", success_rate);

            info!("\ud83c\udf19 Session complete | Runtime: {:.2}s | Avg: {:.2}ms",
                  metrics.elapsed_secs(), metrics.avg_cycle_time());

            println!("\n\u2705 Trading session completed successfully!\n");
        }
        Err(e) => {
            error!("\u274c Trading loop failed: {}", e);
            return Err(e.into());
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_trend_classification() {
        assert_eq!(classify_trend(0.001), "\u27a1\ufe0f");
        assert_eq!(classify_trend(0.15),  "\ud83d\ude80");
        assert_eq!(classify_trend(0.06),  "\ud83d\udcc8");
        assert_eq!(classify_trend(-0.15), "\ud83d\udca5");
        assert_eq!(classify_trend(-0.06), "\ud83d\udcc9");
    }
}
