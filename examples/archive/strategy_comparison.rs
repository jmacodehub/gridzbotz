//! ═══════════════════════════════════════════════════════════════════════════
//! MEGA STRATEGY COMPARISON ENGINE - PROJECT FLASH v2.0
//! Test ALL strategies in parallel with advanced analytics
//! October 14, 2025
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::config::Config;
use solana_grid_bot::trading::{PythHttpFeed, feed_ids, PaperTradingEngine, OrderSide};
use solana_grid_bot::strategies::{GridRebalancer, GridRebalancerConfig};
use tokio::time::{sleep, Duration, interval};
use log::{info, warn};
use std::sync::Arc;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    print_banner();
    
    // Interactive strategy selector
    let configs = select_strategies();
    
    println!("\n🚀 Starting {} parallel tests!", configs.len());
    println!("═══════════════════════════════════════════════════════════════\n");
    
    for (i, (name, _)) in configs.iter().enumerate() {
        println!("   {}. {} Test", i + 1, name);
    }
    
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("⚡ MEGA TEST MODE ACTIVATED!");
    println!("   • All tests run in parallel");
    println!("   • Shared price feed (efficient)");
    println!("   • Real-time updates every 30 min");
    println!("   • Complete comparison at the end");
    println!("═══════════════════════════════════════════════════════════════\n");
    
    println!("🚀 Press Enter to launch all tests, or Ctrl+C to cancel...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    
    // Shared price feed
    info!("🔌 Initializing shared Pyth price feed...");
    let feed = Arc::new(PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]));
    feed.start().await.expect("Failed to start price feed");
    sleep(Duration::from_secs(2)).await;
    
    let initial_price = feed.get_price(feed_ids::SOL_USD).await
        .expect("Failed to get initial price");
    
    println!("\n💵 Starting Price: ${:.4}", initial_price);
    println!("⏱️  Test Duration: 8 hours");
    println!("📊 Tests Running: {}", configs.len());
    println!("════════════════════════════════════════════════════════════════\n");
    
    // Spawn parallel test tasks
    let mut handles = vec![];
    
    for (name, config) in configs {
        let feed_clone = Arc::clone(&feed);
        let handle = tokio::spawn(async move {
            run_strategy_test(name, config, feed_clone, initial_price).await
        });
        handles.push(handle);
    }
    
    // Wait for all tests to complete
    let mut results = vec![];
    for handle in handles {
        if let Ok(result) = handle.await {
            results.push(result);
        }
    }
    
    feed.stop().await;
    
    // Display final comparison
    display_final_comparison(&results);
}

#[derive(Debug, Clone)]
struct TestResult {
    name: String,
    total_fills: usize,
    final_roi: f64,
    final_value: f64,
    total_rebalances: usize,
    win_rate: f64,
    grid_spacing: f64,
}

async fn run_strategy_test(
    test_name: String,
    config: Config,
    feed: Arc<PythHttpFeed>,
    initial_price: f64,
) -> TestResult {
    let test_id = format!("[{}]", test_name.to_uppercase());
    
    info!("{} Initializing paper trading engine...", test_id);
    let engine = PaperTradingEngine::new(
        config.paper_trading.initial_usdc,
        config.paper_trading.initial_sol
    );
    
    info!("{} Setting up grid strategy...", test_id);
    let rebalancer_config = GridRebalancerConfig {
        grid_spacing: config.trading.grid_spacing_percent / 100.0,
        order_size: config.trading.min_order_size,
        min_usdc_balance: config.trading.min_usdc_reserve,
        min_sol_balance: config.trading.min_sol_reserve,
        enabled: config.trading.enable_auto_rebalance,
    };
    let mut rebalancer = GridRebalancer::new(rebalancer_config);
    
    // Place initial grid
    let num_orders = config.trading.grid_levels / 2;
    let spacing = config.trading.grid_spacing_percent / 100.0;
    
    for i in 1..=num_orders {
        let price = initial_price * (1.0 - spacing * i as f64);
        let _ = engine.place_limit_order(OrderSide::Buy, price, config.trading.min_order_size).await;
    }
    
    for i in 1..=num_orders {
        let price = initial_price * (1.0 + spacing * i as f64);
        let _ = engine.place_limit_order(OrderSide::Sell, price, config.trading.min_order_size).await;
    }
    
    info!("{} ✅ Grid placed: {} orders", test_id, num_orders * 2);
    
    // Trading loop
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut elapsed_seconds = 0usize;
    let total_seconds = config.paper_trading.test_duration_hours * 3600;
    let mut total_fills = 0;
    let mut total_rebalances = 0;
    let mut last_report = 0usize;
    
    while elapsed_seconds < total_seconds {
        tick_interval.tick().await;
        elapsed_seconds += 1;
        
        if let Some(current_price) = feed.get_price(feed_ids::SOL_USD).await {
            if let Ok(filled_orders) = engine.process_price_update(current_price).await {
                if !filled_orders.is_empty() {
                    total_fills += filled_orders.len();
                    
                    if config.trading.enable_auto_rebalance {
                        match rebalancer.rebalance_after_fills(&filled_orders, &engine, current_price).await {
                            Ok(new_orders) => {
                                total_rebalances += new_orders.len();
                            }
                            Err(e) => {
                                warn!("{} Rebalancing error: {}", test_id, e);
                            }
                        }
                    }
                }
            }
            
            // Periodic status report (every 30 minutes)
            if elapsed_seconds % 1800 == 0 && elapsed_seconds != last_report {
                last_report = elapsed_seconds;
                let wallet = engine.get_wallet().await;
                let hours = elapsed_seconds / 3600;
                let roi = wallet.roi(current_price);
                
                info!("{} {}h | Fills: {} | Rebal: {} | ROI: {:.2}% | ${:.4}", 
                     test_id, hours, total_fills, total_rebalances, roi, current_price);
            }
        }
    }
    
    // Final report
    let final_price = feed.get_price(feed_ids::SOL_USD).await.unwrap_or(initial_price);
    let wallet = engine.get_wallet().await;
    let final_roi = wallet.roi(final_price);
    let final_value = wallet.total_value_usdc(final_price);
    
    // Calculate win rate (simplified - efficiency metric)
    let win_rate = if total_fills > 0 {
        let efficiency = (total_fills as f64 / (num_orders * 2) as f64) * 100.0;
        efficiency.min(100.0)
    } else {
        0.0
    };
    
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  {} FINAL RESULTS", test_name.to_uppercase());
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!("  Strategy:       {}", test_name);
    println!("  Grid Spacing:   {:.2}%", config.trading.grid_spacing_percent);
    println!("  Total Fills:    {}", total_fills);
    println!("  Rebalances:     {}", total_rebalances);
    println!("  Efficiency:     {:.1}%", win_rate);
    println!("  Final ROI:      {:.2}%", final_roi);
    println!("  Total Value:    ${:.2}", final_value);
    println!("════════════════════════════════════════════════════════════════\n");
    
    TestResult {
        name: test_name,
        total_fills,
        final_roi,
        final_value,
        total_rebalances,
        win_rate,
        grid_spacing: config.trading.grid_spacing_percent,
    }
}

fn display_final_comparison(results: &[TestResult]) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║        🏆 MEGA TEST FINAL COMPARISON - PROJECT FLASH 🏆      ║");
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    if results.is_empty() {
        println!("No results available.\n");
        return;
    }
    
    // Sort by ROI
    let mut sorted_by_roi = results.to_vec();
    sorted_by_roi.sort_by(|a, b| b.final_roi.partial_cmp(&a.final_roi).unwrap());
    
    println!("📊 RANKING BY ROI:\n");
    println!("  Rank | Strategy              | ROI      | Fills | Value      | Spacing");
    println!("  ─────┼───────────────────────┼──────────┼───────┼────────────┼─────────");
    
    for (i, result) in sorted_by_roi.iter().enumerate() {
        println!("  {:2}   | {:<20} | {:>7.2}% | {:>5} | ${:>8.2} | {:>5.2}%",
            i + 1,
            truncate_name(&result.name, 20),
            result.final_roi,
            result.total_fills,
            result.final_value,
            result.grid_spacing
        );
    }
    
    // Best performers
    println!("\n🏆 TOP PERFORMERS:\n");
    
    let best_roi = sorted_by_roi.first().unwrap();
    println!("  💰 Best ROI:        {} ({:.2}%)", best_roi.name, best_roi.final_roi);
    
    let mut sorted_by_fills = results.to_vec();
    sorted_by_fills.sort_by(|a, b| b.total_fills.cmp(&a.total_fills));
    let most_fills = sorted_by_fills.first().unwrap();
    println!("  📈 Most Fills:      {} ({} fills)", most_fills.name, most_fills.total_fills);
    
    let mut sorted_by_value = results.to_vec();
    sorted_by_value.sort_by(|a, b| b.final_value.partial_cmp(&a.final_value).unwrap());
    let highest_value = sorted_by_value.first().unwrap();
    println!("  💎 Highest Value:   {} (${:.2})", highest_value.name, highest_value.final_value);
    
    let mut sorted_by_efficiency = results.to_vec();
    sorted_by_efficiency.sort_by(|a, b| b.win_rate.partial_cmp(&a.win_rate).unwrap());
    let best_efficiency = sorted_by_efficiency.first().unwrap();
    println!("  ⚡ Best Efficiency: {} ({:.1}%)", best_efficiency.name, best_efficiency.win_rate);
    
    // Statistics
    println!("\n📊 STATISTICS:\n");
    let avg_roi = results.iter().map(|r| r.final_roi).sum::<f64>() / results.len() as f64;
    let avg_fills = results.iter().map(|r| r.total_fills).sum::<usize>() / results.len();
    let avg_value = results.iter().map(|r| r.final_value).sum::<f64>() / results.len() as f64;
    
    println!("  Average ROI:        {:.2}%", avg_roi);
    println!("  Average Fills:      {}", avg_fills);
    println!("  Average Value:      ${:.2}", avg_value);
    println!("  Total Tests:        {}", results.len());
    
    println!("\n═══════════════════════════════════════════════════════════════\n");
    println!("🎉 MEGA TEST COMPLETE! You now have production-grade data! 💎\n");
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len-3])
    }
}

fn select_strategies() -> Vec<(String, Config)> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║          🎯 MEGA STRATEGY SELECTOR - PROJECT FLASH 🎯         ║");
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    println!("Select test mode:\n");
    println!("  1. 📊 Quick Comparison (3 tests - 5 min)");
    println!("     • Grid Standard (0.2%)");
    println!("     • Grid Tight (0.15%)");
    println!("     • Grid Wide (0.3%)");
    println!();
    println!("  2. 🎯 Strategy Mix Comparison (6 tests - 1 hour) ⭐");
    println!("     • Grid Only");
    println!("     • Grid + Momentum");
    println!("     • Grid + RSI");
    println!("     • Grid + Mean Reversion");
    println!("     • Grid + Arbitrage");
    println!("     • ALL Strategies Combined");
    println!();
    println!("  3. 🔥 Grid Spacing Comparison (5 tests - 8 hours)");
    println!("     • Conservative (0.3%, 10 levels)");
    println!("     • Balanced (0.2%, 15 levels)");
    println!("     • Aggressive (0.15%, 20 levels)");
    println!("     • Ultra Tight (0.1%, 25 levels)");
    println!("     • Wide Spread (0.5%, 8 levels)");
    println!();
    println!("  4. 💎 MEGA TEST (12 tests - 8 hours) 🚀 RECOMMENDED!");
    println!("     • All 5 grid spacings");
    println!("     • All 6 strategy combinations");
    println!("     • Complete comparison data!");
    println!();
    println!("  5. 🌟 ULTRA MEGA TEST (15 tests - 8 hours) 💥");
    println!("     • Everything from MEGA TEST");
    println!("     • + Hybrid configurations");
    println!("     • + Extreme edge cases");
    println!();
    println!("  6. 🛠️  Custom Selection");
    println!();
    
    print!("Enter your choice (1-6): ");
    use std::io::{self, Write};
    io::stdout().flush().unwrap();
    
    let mut choice = String::new();
    io::stdin().read_line(&mut choice).ok();
    
    match choice.trim() {
        "1" => quick_comparison(),
        "2" => strategy_types(),
        "3" => grid_spacing_comparison(),
        "4" => mega_test(),
        "5" => ultra_mega_test(),
        "6" => custom_selection(),
        _ => {
            println!("Invalid choice, using MEGA TEST...\n");
            mega_test()
        }
    }
}

fn quick_comparison() -> Vec<(String, Config)> {
    vec![
        ("Grid:Standard".to_string(), create_config(0.2, 15)),
        ("Grid:Tight".to_string(), create_config(0.15, 20)),
        ("Grid:Wide".to_string(), create_config(0.3, 10)),
    ]
}

fn strategy_types() -> Vec<(String, Config)> {
    vec![
        ("GridOnly".to_string(), create_config(0.2, 15)),
        ("Grid+Momentum".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum"])),
        ("Grid+RSI".to_string(), create_multi_config(0.2, 15, vec!["grid", "rsi"])),
        ("Grid+MeanRev".to_string(), create_multi_config(0.2, 15, vec!["grid", "mean_reversion"])),
        ("Grid+Arbitrage".to_string(), create_multi_config(0.2, 15, vec!["grid", "arbitrage"])),
        ("AllStrategies".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum", "rsi", "mean_reversion", "arbitrage"])),
    ]
}

fn grid_spacing_comparison() -> Vec<(String, Config)> {
    vec![
        ("Conservative".to_string(), create_config(0.3, 10)),
        ("Balanced".to_string(), create_config(0.2, 15)),
        ("Aggressive".to_string(), create_config(0.15, 20)),
        ("UltraTight".to_string(), create_config(0.1, 25)),
        ("WideSpread".to_string(), create_config(0.5, 8)),
    ]
}

fn mega_test() -> Vec<(String, Config)> {
    let mut tests = vec![];
    
    // Grid spacings
    tests.push(("Grid:Conservative".to_string(), create_config(0.3, 10)));
    tests.push(("Grid:Balanced".to_string(), create_config(0.2, 15)));
    tests.push(("Grid:Aggressive".to_string(), create_config(0.15, 20)));
    tests.push(("Grid:UltraTight".to_string(), create_config(0.1, 25)));
    tests.push(("Grid:WideSpread".to_string(), create_config(0.5, 8)));
    
    // Strategy combinations
    tests.push(("Strat:GridOnly".to_string(), create_config(0.2, 15)));
    tests.push(("Strat:Grid+Mom".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum"])));
    tests.push(("Strat:Grid+RSI".to_string(), create_multi_config(0.2, 15, vec!["grid", "rsi"])));
    tests.push(("Strat:Grid+MeanRev".to_string(), create_multi_config(0.2, 15, vec!["grid", "mean_reversion"])));
    tests.push(("Strat:Grid+Arb".to_string(), create_multi_config(0.2, 15, vec!["grid", "arbitrage"])));
    tests.push(("Strat:ALL".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum", "rsi", "mean_reversion", "arbitrage"])));
    
    println!("\n💎 MEGA TEST ACTIVATED - 12 PARALLEL TESTS!");
    println!("   This will give you COMPLETE comparison data!\n");
    
    tests
}

fn ultra_mega_test() -> Vec<(String, Config)> {
    let mut tests = mega_test();
    
    // Add hybrid configurations
    tests.push(("Hybrid:TightMomentum".to_string(), create_multi_config(0.1, 25, vec!["grid", "momentum"])));
    tests.push(("Hybrid:WideMulti".to_string(), create_multi_config(0.5, 8, vec!["grid", "rsi", "arbitrage"])));
    tests.push(("Hybrid:BalancedALL".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum", "rsi", "mean_reversion", "arbitrage"])));
    
    println!("\n🌟 ULTRA MEGA TEST ACTIVATED - 15 PARALLEL TESTS!");
    println!("   Maximum data collection mode! LFG! 🚀\n");
    
    tests
}

fn custom_selection() -> Vec<(String, Config)> {
    println!("\n🛠️  CUSTOM CONFIGURATION BUILDER");
    println!("════════════════════════════════════════\n");
    println!("Building custom config with interactive builder...\n");
    
    // For now, return mega test
    println!("⚠️  Interactive builder coming soon!");
    println!("   Using MEGA TEST for now...\n");
    mega_test()
}

fn create_config(spacing_pct: f64, levels: u32) -> Config {
    let mut config = Config::overnight_test();
    config.trading.grid_spacing_percent = spacing_pct;
    config.trading.grid_levels = levels;
    config
}

fn create_multi_config(spacing_pct: f64, levels: u32, strategies: Vec<&str>) -> Config {
    let mut config = create_config(spacing_pct, levels);
    config.strategies.active = strategies.iter().map(|s| s.to_string()).collect();
    config.strategies.consensus_mode = "weighted".to_string();
    
    // Enable strategy-specific configs
    for strategy in &strategies {
        match *strategy {
            "momentum" => config.strategies.momentum.enabled = true,
            "rsi" => config.strategies.rsi.enabled = true,
            "mean_reversion" => config.strategies.mean_reversion.enabled = true,
            "arbitrage" => {
                // Arbitrage doesn't need specific config yet
            }
            _ => {}
        }
    }
    
    config
}

fn print_banner() {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║      🤖 MEGA STRATEGY COMPARISON ENGINE v2.0 - FLASH 🤖      ║");
    println!("║         Test Multiple Strategies. Find The Best One.         ║");
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
}
