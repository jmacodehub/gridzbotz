//! ═══════════════════════════════════════════════════════════════════════════
//! 🌟 GIGA ULTRA MEGA TEST - PROJECT FLASH 🌟
//! 20+ parallel tests with enhanced analytics, auto-save, and beautiful output
//! October 14, 2025
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::config::Config;
use solana_grid_bot::trading::{PythHttpFeed, feed_ids, PaperTradingEngine, OrderSide};
use solana_grid_bot::strategies::{GridRebalancer, GridRebalancerConfig};
use tokio::time::{sleep, Duration, interval, Instant};
use log::{info, warn};
use std::sync::Arc;
use serde::{Deserialize, Serialize};
use colored::*;
use std::fs;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    print_giga_banner();
    
    // Create results directory
    fs::create_dir_all("results").ok();
    
    // Interactive strategy selector
    let configs = select_giga_strategies();
    
    println!("\n🚀 Starting {} parallel GIGA tests!", configs.len());
    println!("═══════════════════════════════════════════════════════════════\n");
    
    for (i, (name, _)) in configs.iter().enumerate() {
        println!("   {:>2}. {} Test", i + 1, name.bright_cyan());
    }
    
    println!("\n═══════════════════════════════════════════════════════════════");
    println!("⚡ {} ACTIVATED!", "GIGA ULTRA MEGA MODE".bright_green().bold());
    println!("   • {} running simultaneously", format!("{} tests", configs.len()).bright_yellow());
    println!("   • {} for efficiency", "Shared price feed".bright_white());
    println!("   • {} every 30 min", "Real-time updates".bright_white());
    println!("   • {} to JSON + CSV", "Auto-save results".bright_white());
    println!("   • {} tracking", "Advanced metrics".bright_white());
    println!("   • {} output", "Beautiful colored".bright_magenta());
    println!("═══════════════════════════════════════════════════════════════\n");
    
    println!("🚀 Press {} to launch GIGA test, or Ctrl+C to cancel...", "Enter".bright_green().bold());
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    
    let start_time = Instant::now();
    
    // Shared price feed
    info!("🔌 Initializing shared Pyth price feed...");
    let feed = Arc::new(PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]));
    feed.start().await.expect("Failed to start price feed");
    sleep(Duration::from_secs(2)).await;
    
    let initial_price = feed.get_price(feed_ids::SOL_USD).await
        .expect("Failed to get initial price");
    
    println!("\n💵 Starting Price: ${}", format!("{:.4}", initial_price).bright_yellow());
    println!("⏱️  Test Duration: {} hours", "8".bright_cyan());
    println!("📊 Tests Running: {}", format!("{}", configs.len()).bright_green());
    println!("🎯 Target: {} data points", "Maximum".bright_magenta());
    println!("════════════════════════════════════════════════════════════════\n");
    
    // Spawn parallel test tasks
    let mut handles = vec![];
    
    for (name, config) in configs {
        let feed_clone = Arc::clone(&feed);
        let handle = tokio::spawn(async move {
            run_giga_test(name, config, feed_clone, initial_price).await
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
    
    let total_duration = start_time.elapsed();
    
    // Display final comparison with auto-save
    display_giga_results(&results, initial_price, total_duration);
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct GigaTestResult {
    name: String,
    total_fills: usize,
    final_roi: f64,
    final_value: f64,
    total_rebalances: usize,
    efficiency: f64,
    grid_spacing: f64,
    grid_levels: u32,
    strategies: Vec<String>,
    max_drawdown: f64,
    sharpe_ratio: f64,
    volatility: f64,
    price_range: (f64, f64),
    duration_seconds: u64,
}

async fn run_giga_test(
    test_name: String,
    config: Config,
    feed: Arc<PythHttpFeed>,
    initial_price: f64,
) -> GigaTestResult {
    let test_id = format!("[{}]", test_name.to_uppercase());
    
    info!("{} {} paper trading engine...", test_id, "Initializing".bright_green());
    let engine = PaperTradingEngine::new(
        config.paper_trading.initial_usdc,
        config.paper_trading.initial_sol
    );
    
    info!("{} {} grid strategy...", test_id, "Setting up".bright_cyan());
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
    
    // Trading loop with enhanced tracking
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut elapsed_seconds = 0u64;
    let total_seconds = config.paper_trading.test_duration_hours * 3600;
    let mut total_fills = 0;
    let mut total_rebalances = 0;
    let mut last_report = 0u64;
    let mut min_price = initial_price;
    let mut max_price = initial_price;
    let mut price_samples = vec![];
    let mut peak_value = config.paper_trading.initial_usdc + (config.paper_trading.initial_sol * initial_price);
    let mut max_drawdown = 0.0;
    
    let start_test = Instant::now();
    
    while elapsed_seconds < total_seconds {
        tick_interval.tick().await;
        elapsed_seconds += 1;
        
        if let Some(current_price) = feed.get_price(feed_ids::SOL_USD).await {
            // Track price range
            min_price = min_price.min(current_price);
            max_price = max_price.max(current_price);
            
            // Sample price every 10 seconds for volatility
            if elapsed_seconds % 10 == 0 {
                price_samples.push(current_price);
            }
            
            // Track drawdown
            let wallet = engine.get_wallet().await;
            let current_value = wallet.total_value_usdc(current_price);
            if current_value > peak_value {
                peak_value = current_value;
            }
            let drawdown = ((peak_value - current_value) / peak_value) * 100.0;
            if drawdown > max_drawdown {
                max_drawdown = drawdown;
            }
            
            // Process orders
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
    
    let duration_seconds = start_test.elapsed().as_secs();
    
    // Final calculations
    let final_price = feed.get_price(feed_ids::SOL_USD).await.unwrap_or(initial_price);
    let wallet = engine.get_wallet().await;
    let final_roi = wallet.roi(final_price);
    let final_value = wallet.total_value_usdc(final_price);
    
    // Calculate volatility
    let volatility = if price_samples.len() > 1 {
        let mean = price_samples.iter().sum::<f64>() / price_samples.len() as f64;
        let variance = price_samples.iter()
            .map(|p| (p - mean).powi(2))
            .sum::<f64>() / price_samples.len() as f64;
        variance.sqrt()
    } else {
        0.0
    };
    
    // Calculate Sharpe ratio (simplified)
    let sharpe_ratio = if max_drawdown > 0.0 {
        final_roi / max_drawdown
    } else {
        final_roi
    };
    
    // Calculate efficiency
    let efficiency = if num_orders > 0 {
        (total_fills as f64 / (num_orders * 2) as f64) * 100.0
    } else {
        0.0
    };
    
    // Print final result
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║  {} FINAL RESULTS", test_name.to_uppercase().bright_yellow());
    println!("╚═══════════════════════════════════════════════════════════════╝");
    println!("  Strategy:       {}", test_name.bright_cyan());
    println!("  Grid Spacing:   {}%", format!("{:.2}", config.trading.grid_spacing_percent).bright_green());
    println!("  Grid Levels:    {}", config.trading.grid_levels);
    println!("  Total Fills:    {}", format!("{}", total_fills).bright_yellow());
    println!("  Rebalances:     {}", total_rebalances);
    println!("  Efficiency:     {}%", format!("{:.1}", efficiency).bright_magenta());
    println!("  Final ROI:      {}%", format!("{:.2}", final_roi).bright_green());
    println!("  Total Value:    ${}", format!("{:.2}", final_value).bright_yellow());
    println!("  Max Drawdown:   {}%", format!("{:.2}", max_drawdown).bright_red());
    println!("  Sharpe Ratio:   {}", format!("{:.2}", sharpe_ratio).bright_cyan());
    println!("  Volatility:     ${}", format!("{:.4}", volatility).bright_white());
    println!("  Price Range:    ${:.4} - ${:.4}", min_price, max_price);
    println!("════════════════════════════════════════════════════════════════\n");
    
    GigaTestResult {
        name: test_name,
        total_fills,
        final_roi,
        final_value,
        total_rebalances,
        efficiency,
        grid_spacing: config.trading.grid_spacing_percent,
        grid_levels: config.trading.grid_levels,
        strategies: config.strategies.active.clone(),
        max_drawdown,
        sharpe_ratio,
        volatility,
        price_range: (min_price, max_price),
        duration_seconds,
    }
}

fn display_giga_results(results: &[GigaTestResult], initial_price: f64, duration: Duration) {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║        {} 🏆      ║", "🏆 GIGA TEST FINAL COMPARISON - PROJECT FLASH".bright_yellow().bold());
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    if results.is_empty() {
        println!("{}No results available.\n", "⚠️  ".bright_red());
        return;
    }
    
    // Sort by ROI
    let mut sorted_by_roi = results.to_vec();
    sorted_by_roi.sort_by(|a, b| b.final_roi.partial_cmp(&a.final_roi).unwrap());
    
    println!("{}\n", "📊 RANKING BY ROI:".bright_cyan().bold());
    println!("  {} | {:<20} | {:>8} | {:>5} | {:>10} | {:>7} | {:>7}",
        "Rank".bright_white(),
        "Strategy".bright_white(),
        "ROI".bright_white(),
        "Fills".bright_white(),
        "Value".bright_white(),
        "Spacing".bright_white(),
        "Sharpe".bright_white()
    );
    println!("  ─────┼──────────────────────┼──────────┼───────┼────────────┼─────────┼─────────");
    
    for (i, result) in sorted_by_roi.iter().take(20).enumerate() {
        let rank_color = match i {
            0 => "bright_yellow",
            1 => "bright_cyan",
            2 => "bright_magenta",
            _ => "white",
        };
        
        let roi_str = format!("{:>7.2}%", result.final_roi);
        let roi_colored = if result.final_roi > 0.0 {
            roi_str.bright_green()
        } else {
            roi_str.bright_red()
        };
        
        println!("  {:>4} │ {:<20} │ {} │ {:>5} │ ${:>8.2} │ {:>6.2}% │ {:>7.2}",
            format!("#{}", i + 1).color(rank_color),
            truncate_name(&result.name, 20),
            roi_colored,
            result.total_fills,
            result.final_value,
            result.grid_spacing,
            result.sharpe_ratio
        );
    }
    
    // Best performers
    println!("\n{}\n", "🏆 TOP PERFORMERS:".bright_yellow().bold());
    
    let best_roi = sorted_by_roi.first().unwrap();
    println!("  💰 {:<18} {} ({:.2}%)", 
        "Best ROI:".bright_white(),
        best_roi.name.bright_green().bold(),
        best_roi.final_roi
    );
    
    let mut sorted_by_fills = results.to_vec();
    sorted_by_fills.sort_by(|a, b| b.total_fills.cmp(&a.total_fills));
    let most_fills = sorted_by_fills.first().unwrap();
    println!("  📈 {:<18} {} ({} fills)", 
        "Most Fills:".bright_white(),
        most_fills.name.bright_cyan().bold(),
        most_fills.total_fills
    );
    
    let mut sorted_by_value = results.to_vec();
    sorted_by_value.sort_by(|a, b| b.final_value.partial_cmp(&a.final_value).unwrap());
    let highest_value = sorted_by_value.first().unwrap();
    println!("  💎 {:<18} {} (${:.2})", 
        "Highest Value:".bright_white(),
        highest_value.name.bright_magenta().bold(),
        highest_value.final_value
    );
    
    let mut sorted_by_sharpe = results.to_vec();
    sorted_by_sharpe.sort_by(|a, b| b.sharpe_ratio.partial_cmp(&a.sharpe_ratio).unwrap());
    let best_sharpe = sorted_by_sharpe.first().unwrap();
    println!("  ⚡ {:<18} {} ({:.2})", 
        "Best Sharpe:".bright_white(),
        best_sharpe.name.bright_yellow().bold(),
        best_sharpe.sharpe_ratio
    );
    
    let mut sorted_by_efficiency = results.to_vec();
    sorted_by_efficiency.sort_by(|a, b| b.efficiency.partial_cmp(&a.efficiency).unwrap());
    let best_efficiency = sorted_by_efficiency.first().unwrap();
    println!("  🎯 {:<18} {} ({:.1}%)", 
        "Best Efficiency:".bright_white(),
        best_efficiency.name.bright_cyan().bold(),
        best_efficiency.efficiency
    );
    
    // Statistics
    println!("\n{}\n", "📊 OVERALL STATISTICS:".bright_magenta().bold());
    let avg_roi = results.iter().map(|r| r.final_roi).sum::<f64>() / results.len() as f64;
    let avg_fills = results.iter().map(|r| r.total_fills).sum::<usize>() / results.len();
    let avg_value = results.iter().map(|r| r.final_value).sum::<f64>() / results.len() as f64;
    let avg_sharpe = results.iter().map(|r| r.sharpe_ratio).sum::<f64>() / results.len() as f64;
    
    println!("  Average ROI:        {}", format!("{:.2}%", avg_roi).bright_green());
    println!("  Average Fills:      {}", format!("{}", avg_fills).bright_yellow());
    println!("  Average Value:      {}", format!("${:.2}", avg_value).bright_cyan());
    println!("  Average Sharpe:     {}", format!("{:.2}", avg_sharpe).bright_magenta());
    println!("  Total Tests:        {}", format!("{}", results.len()).bright_white());
    println!("  Test Duration:      {}", format!("{:.1} hours", duration.as_secs_f64() / 3600.0).bright_cyan());
    println!("  Starting Price:     {}", format!("${:.4}", initial_price).bright_yellow());
    
    // Save results
    save_results(results, initial_price, duration);
    
    println!("\n═══════════════════════════════════════════════════════════════\n");
    println!("{} You now have {} data! 💎\n", 
        "🎉 GIGA TEST COMPLETE!".bright_green().bold(),
        "production-grade".bright_yellow()
    );
}

fn save_results(results: &[GigaTestResult], initial_price: f64, duration: Duration) {
    use chrono::Local;
    
    let timestamp = Local::now().format("%Y%m%d_%H%M%S");
    
    // Save JSON
    let json_filename = format!("results/giga_test_{}.json", timestamp);
    match serde_json::to_string_pretty(&results) {
        Ok(json_data) => {
            if let Err(e) = fs::write(&json_filename, json_data) {
                println!("{}Failed to save JSON: {}", "⚠️  ".bright_red(), e);
            } else {
                println!("\n💾 Results saved to: {}", json_filename.bright_green());
            }
        }
        Err(e) => println!("{}Failed to serialize JSON: {}", "⚠️  ".bright_red(), e),
    }
    
    // Save CSV
    let csv_filename = format!("results/giga_test_{}.csv", timestamp);
    let mut csv_content = String::from("Rank,Strategy,ROI,Fills,Value,Rebalances,Efficiency,Spacing,Levels,Sharpe,MaxDrawdown,Volatility\n");
    
    let mut sorted = results.to_vec();
    sorted.sort_by(|a, b| b.final_roi.partial_cmp(&a.final_roi).unwrap());
    
    for (i, result) in sorted.iter().enumerate() {
        csv_content.push_str(&format!(
            "{},{},{:.2},{},{:.2},{},{:.2},{:.2},{},{:.2},{:.2},{:.4}\n",
            i + 1,
            result.name,
            result.final_roi,
            result.total_fills,
            result.final_value,
            result.total_rebalances,
            result.efficiency,
            result.grid_spacing,
            result.grid_levels,
            result.sharpe_ratio,
            result.max_drawdown,
            result.volatility
        ));
    }
    
    if let Err(e) = fs::write(&csv_filename, csv_content) {
        println!("{}Failed to save CSV: {}", "⚠️  ".bright_red(), e);
    } else {
        println!("📊 CSV saved to: {}", csv_filename.bright_green());
    }
    
    // Save summary
    let summary_filename = format!("results/giga_test_{}_summary.txt", timestamp);
    let summary = format!(
        "GIGA TEST SUMMARY\n\
        ═══════════════════════════════════════════════════════════════\n\
        \n\
        Test Timestamp: {}\n\
        Total Tests: {}\n\
        Test Duration: {:.1} hours\n\
        Starting Price: ${:.4}\n\
        \n\
        BEST PERFORMERS:\n\
        • Best ROI: {} ({:.2}%)\n\
        • Most Fills: {} ({} fills)\n\
        • Highest Value: {} (${:.2})\n\
        \n\
        AVERAGES:\n\
        • Average ROI: {:.2}%\n\
        • Average Fills: {}\n\
        • Average Value: ${:.2}\n\
        \n\
        Results saved to:\n\
        • JSON: {}\n\
        • CSV: {}\n\
        ",
        Local::now().format("%Y-%m-%d %H:%M:%S"),
        results.len(),
        duration.as_secs_f64() / 3600.0,
        initial_price,
        sorted.first().unwrap().name,
        sorted.first().unwrap().final_roi,
        sorted.iter().max_by_key(|r| r.total_fills).unwrap().name,
        sorted.iter().max_by_key(|r| r.total_fills).unwrap().total_fills,
        sorted.iter().max_by(|a, b| a.final_value.partial_cmp(&b.final_value).unwrap()).unwrap().name,
        sorted.iter().max_by(|a, b| a.final_value.partial_cmp(&b.final_value).unwrap()).unwrap().final_value,
        results.iter().map(|r| r.final_roi).sum::<f64>() / results.len() as f64,
        results.iter().map(|r| r.total_fills).sum::<usize>() / results.len(),
        results.iter().map(|r| r.final_value).sum::<f64>() / results.len() as f64,
        json_filename,
        csv_filename
    );
    
    if let Err(e) = fs::write(&summary_filename, summary) {
        println!("{}Failed to save summary: {}", "⚠️  ".bright_red(), e);
    } else {
        println!("📄 Summary saved to: {}", summary_filename.bright_green());
    }
}

fn select_giga_strategies() -> Vec<(String, Config)> {
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║          {}         ║", "🎯 GIGA STRATEGY SELECTOR - PROJECT FLASH 🎯".bright_cyan());
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    println!("Select GIGA test mode:\n");
    println!("  {}. 📊 Quick Test (3 tests - 5 min)", "1".bright_green());
    println!("     • Grid Standard (0.2%)");
    println!("     • Grid Tight (0.15%)");
    println!("     • Grid Wide (0.3%)");
    println!();
    println!("  {}. 🎯 Strategy Mix (6 tests - 1 hour)", "2".bright_cyan());
    println!("     • Grid Only");
    println!("     • Grid + Momentum");
    println!("     • Grid + RSI");
    println!("     • Grid + Mean Reversion");
    println!("     • Grid + Arbitrage");
    println!("     • ALL Strategies Combined");
    println!();
    println!("  {}. 🔥 Grid Spacing (5 tests - 8 hours)", "3".bright_magenta());
    println!("     • Conservative (0.3%, 10 levels)");
    println!("     • Balanced (0.2%, 15 levels)");
    println!("     • Aggressive (0.15%, 20 levels)");
    println!("     • Ultra Tight (0.1%, 25 levels)");
    println!("     • Wide Spread (0.5%, 8 levels)");
    println!();
    println!("  {}. 💎 MEGA TEST (12 tests - 8 hours)", "4".bright_yellow());
    println!("     • All 5 grid spacings");
    println!("     • All 6 strategy combinations");
    println!();
    println!("  {}. 🌟 ULTRA MEGA (15 tests - 8 hours)", "5".bright_red());
    println!("     • MEGA TEST");
    println!("     • + 3 Hybrid configurations");
    println!();
    println!("  {}. 🔥💎🚀 GIGA MODE (20 tests - 8 hours) {}", "6".bright_green().bold(), "⭐ RECOMMENDED!".bright_yellow());
    println!("     • ULTRA MEGA TEST");
    println!("     • + 5 Extreme configurations");
    println!("     • {} DATA!", "MAXIMUM".bright_red().bold());
    println!();
    
    print!("Enter your choice (1-6): ");
    use std::io::{self, Write};
    io::stdout().flush().unwrap();
    
    let mut choice = String::new();
    io::stdin().read_line(&mut choice).ok();
    
    match choice.trim() {
        "1" => quick_test(),
        "2" => strategy_mix(),
        "3" => grid_spacing(),
        "4" => mega_test(),
        "5" => ultra_mega_test(),
        "6" => giga_mode(),
        _ => {
            println!("\n{}Using GIGA MODE...\n", "Invalid choice, ".bright_red());
            giga_mode()
        }
    }
}

fn quick_test() -> Vec<(String, Config)> {
    vec![
        ("Grid:Standard".to_string(), create_config(0.2, 15)),
        ("Grid:Tight".to_string(), create_config(0.15, 20)),
        ("Grid:Wide".to_string(), create_config(0.3, 10)),
    ]
}

fn strategy_mix() -> Vec<(String, Config)> {
    vec![
        ("GridOnly".to_string(), create_config(0.2, 15)),
        ("Grid+Momentum".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum"])),
        ("Grid+RSI".to_string(), create_multi_config(0.2, 15, vec!["grid", "rsi"])),
        ("Grid+MeanRev".to_string(), create_multi_config(0.2, 15, vec!["grid", "mean_reversion"])),
        ("Grid+Arbitrage".to_string(), create_multi_config(0.2, 15, vec!["grid", "arbitrage"])),
        ("AllStrategies".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum", "rsi", "mean_reversion", "arbitrage"])),
    ]
}

fn grid_spacing() -> Vec<(String, Config)> {
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
    
    println!("\n💎 {} - 12 PARALLEL TESTS!", "MEGA TEST ACTIVATED".bright_yellow().bold());
    println!("   This will give you {} data!\n", "COMPLETE comparison".bright_green());
    
    tests
}

fn ultra_mega_test() -> Vec<(String, Config)> {
    let mut tests = mega_test();
    
    // Add hybrid configurations
    tests.push(("Hybrid:TightMomentum".to_string(), create_multi_config(0.1, 25, vec!["grid", "momentum"])));
    tests.push(("Hybrid:WideMulti".to_string(), create_multi_config(0.5, 8, vec!["grid", "rsi", "arbitrage"])));
    tests.push(("Hybrid:BalancedALL".to_string(), create_multi_config(0.2, 15, vec!["grid", "momentum", "rsi", "mean_reversion", "arbitrage"])));
    
    println!("\n🌟 {} - 15 PARALLEL TESTS!", "ULTRA MEGA TEST ACTIVATED".bright_red().bold());
    println!("   Maximum data collection! {}!\n", "LFG".bright_yellow().bold());
    
    tests
}

fn giga_mode() -> Vec<(String, Config)> {
    let mut tests = ultra_mega_test();
    
    // Add EXTREME configurations
    tests.push(("Extreme:MicroGrid".to_string(), create_config(0.05, 40)));
    tests.push(("Extreme:MacroGrid".to_string(), create_config(1.0, 5)));
    tests.push(("Extreme:MegaTight".to_string(), create_config(0.08, 30)));
    tests.push(("Extreme:UltraWide".to_string(), create_config(0.75, 6)));
    tests.push(("Extreme:MaxLevels".to_string(), create_config(0.15, 35)));
    
    println!("\n🔥💎🚀 {} 🚀💎🔥", "GIGA MODE ACTIVATED".bright_green().bold());
    println!("   {} PARALLEL TESTS!", "20".bright_yellow().bold());
    println!("   {} DATA COLLECTION!", "ULTIMATE".bright_red().bold());
    println!("   {}! 🌟\n", "LET'S FUCKING GOOOOO".bright_magenta().bold());
    
    tests
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
    
    for strategy in &strategies {
        match *strategy {
            "momentum" => config.strategies.momentum.enabled = true,
            "rsi" => config.strategies.rsi.enabled = true,
            "mean_reversion" => config.strategies.mean_reversion.enabled = true,
            "arbitrage" => {}
            _ => {}
        }
    }
    
    config
}

fn truncate_name(name: &str, max_len: usize) -> String {
    if name.len() <= max_len {
        name.to_string()
    } else {
        format!("{}...", &name[..max_len-3])
    }
}

fn print_giga_banner() {
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║                                                               ║");
    println!("║      {}      ║", "🔥 GIGA ULTRA MEGA TEST v3.0 - PROJECT FLASH 🔥".bright_green().bold());
    println!("║         {}         ║", "Test Multiple Strategies. Find The Best One.".bright_cyan());
    println!("║                                                               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
}
