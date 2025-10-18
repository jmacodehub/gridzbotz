//! ═══════════════════════════════════════════════════════════════════════════
//! 🔥💎🚀 GIGA ULTRA MEGA TEST 3.0 - PROJECT FLASH (OCT 2025) 
//! Enhanced for FULL PYTH integration, ultra-flexible grid, and parallelism!
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::trading::{
    Config, get_live_price, live_feed_ids, PaperTradingEngine, OrderSide
};
use solana_grid_bot::strategies::{GridRebalancer, GridRebalancerConfig};
use log::{info, warn, LevelFilter};
use tokio::time::{sleep, interval, Duration, Instant};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use chrono::Utc;
use std::fs::OpenOptions;
use std::io::Write;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::new().filter_level(LevelFilter::Info).init();
    print_giga_banner();

    let configs = select_giga3_strategies();

    let price_feed_id = live_feed_ids::SOL_USD;
    let mut initial_price = 193.5;
    for _ in 0..5 {
        if let Some(price) = get_live_price(price_feed_id).await {
            initial_price = price;
            break;
        }
        sleep(Duration::from_secs(2)).await;
    }

    println!("🚦 Launching {} GIGA 3.0 strategies in parallel!", configs.len());
    let test_duration_secs = 3600; // default: 1 hour
    let start_time = Instant::now();

    let mut handles = vec![];
    for (name, config) in configs.clone() {
        let pfid = price_feed_id;
        let init_price = initial_price;
        let dur_secs = test_duration_secs;
        let handle = tokio::spawn(async move {
            run_giga3_test(name, config, pfid, init_price, dur_secs).await
        });
        handles.push(handle);
    }

    let mut results = vec![];
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }
    display_giga3_results(&results, initial_price, start_time.elapsed());
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GigaTestResult {
    name: String,
    final_roi: f64,
    final_value: f64,
    fills: usize,
    rebalances: usize,
    efficiency: f64,
    grid_spacing: f64,
    grid_levels: u32,
    max_drawdown: f64,
    sharpe_ratio: f64,
    trading_paused_pct: f64,
    log_file: String,
}

async fn run_giga3_test(
    test_name: String,
    config: Config,
    price_feed_id: &'static str,
    initial_price: f64,
    total_seconds: u64,
) -> GigaTestResult {
    let mut engine = PaperTradingEngine::new(&config);
    let mut rebalancer = GridRebalancer::new(GridRebalancerConfig::from_config(&config));
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut elapsed = 0u64;
    let mut last_price = initial_price;
    let mut fills = 0;
    let log_path = format!("logs/giga3_{}_{}.log", test_name, Utc::now().timestamp());
    let mut log = OpenOptions::new().create(true).append(true).open(&log_path).unwrap();

    let mut pause_ticks = 0;
    let mut min_price = initial_price;
    let mut max_price = initial_price;
    let mut peak_value = engine.get_wallet().await.total_value_usdc(initial_price);
    let mut max_drawdown = 0.0;

    while elapsed < total_seconds {
        tick_interval.tick().await;
        elapsed += 1;
        if let Some(price) = get_live_price(price_feed_id).await {
            last_price = price;
            min_price = min_price.min(price);
            max_price = max_price.max(price);
            rebalancer.update_price(price);
            engine.on_price_tick(price).await;
            if let Ok(new_fills) = engine.process_price_update(price).await {
                fills += new_fills.len();
            }
            // Drawdown
            let value = engine.get_wallet().await.total_value_usdc(price);
            if value > peak_value { peak_value = value; }
            let drawdown = ((peak_value - value) / peak_value) * 100.0;
            if drawdown > max_drawdown { max_drawdown = drawdown; }
            // Paused trading?
            if rebalancer.stats().trading_paused { pause_ticks += 1; }
        }
        if elapsed % 30 == 0 {
            writeln!(log, "[{}] price: {:.4} fills: {} state: {:?}", Utc::now(), last_price, fills, rebalancer.stats()).unwrap();
        }
    }
    let wallet = engine.get_wallet().await;
    let final_roi = wallet.roi(last_price);
    let final_value = wallet.total_value_usdc(last_price);

    let stats = rebalancer.stats();

    // Sharpe and efficiency for reporting
    let sharpe = if max_drawdown > 0.0 { final_roi / max_drawdown } else { final_roi };
    let efficiency = if config.trading.grid_levels > 0 {
        (fills as f64 / config.trading.grid_levels as f64) * 100.0
    } else {
        0.0
    };

    GigaTestResult {
        name: test_name,
        final_roi,
        final_value,
        fills,
        rebalances: stats.rebalances,
        efficiency,
        grid_spacing: config.trading.grid_spacing_percent,
        grid_levels: config.trading.grid_levels,
        max_drawdown,
        sharpe_ratio: sharpe,
        trading_paused_pct: (pause_ticks as f64 / total_seconds as f64) * 100.0,
        log_file: log_path,
    }
}

// === STRATEGY SELECTION / LOGIC / CONFIG ===

fn select_giga3_strategies() -> Vec<(String, Config)> {
    // You can copy/adapt from v2.5 for FLASH 3.0 presets or make new ones
    vec![
        ("MaxLevels".to_string(), create_config(0.15, 35)),
        ("MicroGrid".to_string(), create_config(0.05, 40)),
        ("Aggressive".to_string(), create_config(0.15, 20)),
        ("Balanced".to_string(), create_config(0.20, 15)),
        ("UltraWide".to_string(), create_config(0.75, 6)),
        ("Conservative".to_string(), create_config(0.30, 10)),
    ]
}

fn create_config(spacing_pct: f64, levels: u32) -> Config {
    let mut config = Config::overnight_test();
    config.trading.grid_spacing_percent = spacing_pct;
    config.trading.grid_levels = levels;
    config
}

fn display_giga3_results(results: &[GigaTestResult], initial_price: f64, duration: Duration) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║            🏆 GIGA FLASH 3.0 FINAL RESULTS 🏆                ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");

    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| b.final_roi.partial_cmp(&a.final_roi).unwrap());

    println!("{:<10} | {:>8} | {:>8} | {:>10} | {:>8} | {:>7} | {:>7.2}% | {:>8.2}", 
             "Strategy", "ROI", "Fills", "Value", "Drawdown", "Sharpe", "Paused%", "LogFile");
    println!("{:-<90}", "");

    for result in sorted {
        println!("{:<10} | {:>8.2}% | {:>8} | ${:>10.2} | {:>8.2}% | {:>7.2} | {:>7.2}% | {}",
            result.name, result.final_roi, result.fills, result.final_value,
            result.max_drawdown, result.sharpe_ratio, result.trading_paused_pct, result.log_file
        );
    }

    println!("\nTest completed. Duration: {:.1} minutes. Starting price: ${:.2}\n",
             duration.as_secs_f64()/60.0, initial_price);
}

fn print_giga_banner() {
    println!("\n╔═════════════════════════════════════════════════╗");
    println!("║      🔥💎🚀 GIGA FLASH 3.0 ULTRA MEGA TEST     ║");
    println!("╚═════════════════════════════════════════════════╝\n");
}
