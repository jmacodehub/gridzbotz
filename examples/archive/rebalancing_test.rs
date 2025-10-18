//! ═══════════════════════════════════════════════════════════════════════════
//! CONFIG-DRIVEN AUTO-REBALANCING GRID TEST
//! Load settings from TOML file or use presets
//! October 14, 2025
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::trading::{PythHttpFeed, feed_ids, PaperTradingEngine, OrderSide};
use solana_grid_bot::strategies::{GridRebalancer, GridRebalancerConfig};
use solana_grid_bot::config::BotConfig;
use tokio::time::{sleep, Duration, interval};
use log::{info, warn};
use std::env;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    print_banner();
    
    // ═══════════════════════════════════════════════════════════════════════
    // LOAD CONFIGURATION
    // ═══════════════════════════════════════════════════════════════════════
    
    let config = load_config();
    config.display();
    
    println!("\n🚀 Press Enter to start, or Ctrl+C to cancel...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    
    // ═══════════════════════════════════════════════════════════════════════
    // INITIALIZE SYSTEMS
    // ═══════════════════════════════════════════════════════════════════════
    
    info!("🔌 Connecting to Pyth Network...");
    let feed = PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]);
    feed.start().await.expect("Failed to start price feed");
    sleep(Duration::from_secs(2)).await;
    
    info!("🎮 Initializing paper trading engine...");
    let engine = PaperTradingEngine::new(
        config.trading.initial_usdc,
        config.trading.initial_sol
    );
    
    info!("🤖 Initializing grid rebalancer...");
    let rebalancer_config = GridRebalancerConfig {
        grid_spacing: config.grid.spacing_pct,
        order_size: config.grid.order_size,
        min_usdc_balance: config.grid.min_usdc_reserve,
        min_sol_balance: config.grid.min_sol_reserve,
        enabled: config.grid.auto_rebalance,
    };
    let mut rebalancer = GridRebalancer::new(rebalancer_config);
    
    let initial_price = feed.get_price(feed_ids::SOL_USD).await
        .expect("Failed to get initial price");
    
    info!("💵 Current SOL price: ${:.4}\n", initial_price);
    
    // ═══════════════════════════════════════════════════════════════════════
    // PLACE INITIAL GRID
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("📊 SETTING UP GRID FROM CONFIG");
    println!("════════════════════════════════════════");
    
    let mut buy_count = 0;
    for i in 1..=config.grid.num_buy_orders {
        let price = initial_price * (1.0 - config.grid.spacing_pct * i as f64);
        if let Ok(id) = engine.place_limit_order(OrderSide::Buy, price, config.grid.order_size).await {
            buy_count += 1;
            if i <= 3 {
                println!("  ✅ BUY  {} SOL @ ${:.4} ({})", config.grid.order_size, price, id);
            }
        }
    }
    if config.grid.num_buy_orders > 3 {
        println!("  ... and {} more buy orders", config.grid.num_buy_orders - 3);
    }
    
    println!();
    
    let mut sell_count = 0;
    for i in 1..=config.grid.num_sell_orders {
        let price = initial_price * (1.0 + config.grid.spacing_pct * i as f64);
        if let Ok(id) = engine.place_limit_order(OrderSide::Sell, price, config.grid.order_size).await {
            sell_count += 1;
            if i <= 3 {
                println!("  ✅ SELL {} SOL @ ${:.4} ({})", config.grid.order_size, price, id);
            }
        }
    }
    if config.grid.num_sell_orders > 3 {
        println!("  ... and {} more sell orders", config.grid.num_sell_orders - 3);
    }
    
    println!("\n╔═══════════════════════════════════════╗");
    println!("║  ✅ GRID READY - {} ORDERS           ║", buy_count + sell_count);
    println!("╚═══════════════════════════════════════╝");
    println!("  Buy Orders:       {}", buy_count);
    println!("  Sell Orders:      {}", sell_count);
    println!("  Rebalancing:      {}\n", if config.grid.auto_rebalance { "🤖 AUTO" } else { "❌ OFF" });
    
    engine.display_status(initial_price).await;
    
    // ═══════════════════════════════════════════════════════════════════════
    // MAIN TRADING LOOP
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("\n⏱️  STARTING {} HOUR TEST", config.trading.test_duration_hours);
    println!("════════════════════════════════════════");
    println!("  • Auto-rebalancing: {}", if config.grid.auto_rebalance { "✅" } else { "❌" });
    println!("  • Status updates every {}s", config.monitoring.status_interval_sec);
    println!("  • Results saved: {}", if config.monitoring.save_results { "✅" } else { "❌" });
    println!("  • Press Ctrl+C to stop early\n");
    
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut elapsed_seconds = 0;
    let total_seconds = config.trading.test_duration_hours * 3600;
    let mut last_price = initial_price;
    let mut price_high = initial_price;
    let mut price_low = initial_price;
    let mut total_fills = 0;
    let mut total_rebalances = 0;
    
    while elapsed_seconds < total_seconds {
        tick_interval.tick().await;
        elapsed_seconds += 1;
        
        if let Some(current_price) = feed.get_price(feed_ids::SOL_USD).await {
            price_high = price_high.max(current_price);
            price_low = price_low.min(current_price);
            
            // Check for order fills
            if let Ok(filled_orders) = engine.process_price_update(current_price).await {
                if !filled_orders.is_empty() {
                    total_fills += filled_orders.len();
                    
                    println!("\n╔═══════════════════════════════════════╗");
                    println!("║  🎉 {} ORDER(S) FILLED!               ║", filled_orders.len());
                    println!("╚═══════════════════════════════════════╝");
                    println!("  Time:     {}h {}m {}s", 
                        elapsed_seconds / 3600, 
                        (elapsed_seconds % 3600) / 60,
                        elapsed_seconds % 60
                    );
                    println!("  Price:    ${:.4}", current_price);
                    
                    // Show filled orders
                    let trades = engine.get_trade_history(filled_orders.len()).await;
                    for trade in trades {
                        let side = match trade.side {
                            OrderSide::Buy => "BUY ",
                            OrderSide::Sell => "SELL",
                        };
                        println!("  {} {} SOL @ ${:.4} (fee: ${:.4})",
                            side, trade.size, trade.price, trade.fee);
                    }
                    
                    // Auto-rebalance if enabled
                    if config.grid.auto_rebalance {
                        println!("\n🤖 AUTO-REBALANCING...");
                        match rebalancer.rebalance_after_fills(&filled_orders, &engine, current_price).await {
                            Ok(new_orders) => {
                                if !new_orders.is_empty() {
                                    total_rebalances += new_orders.len();
                                    println!("  ✅ Placed {} new order(s)", new_orders.len());
                                } else {
                                    println!("  ℹ️  No rebalancing needed");
                                }
                            }
                            Err(e) => {
                                warn!("  ⚠️  Rebalancing error: {}", e);
                            }
                        }
                    }
                    
                    engine.display_status(current_price).await;
                }
            }
            
            last_price = current_price;
            
            // Status updates
            if elapsed_seconds % config.monitoring.status_interval_sec == 0 {
                let hours = elapsed_seconds / 3600;
                let minutes = (elapsed_seconds % 3600) / 60;
                
                println!("\n╔═══════════════════════════════════════╗");
                println!("║  ⏰ {}h {}m ELAPSED                   ║", hours, minutes);
                println!("╚═══════════════════════════════════════╝");
                println!("  Price:          ${:.4}", current_price);
                println!("  Range:          ${:.4} - ${:.4}", price_low, price_high);
                println!("  Total Fills:    {}", total_fills);
                println!("  Rebalances:     {}", total_rebalances);
                println!();
                
                engine.display_status(current_price).await;
                if config.grid.auto_rebalance {
                    rebalancer.stats().display();
                }
            }
            
            // Mini progress
            if elapsed_seconds % 300 == 0 && elapsed_seconds % config.monitoring.status_interval_sec != 0 {
                print!("  ⏳ {}h {}m | ${:.4} | Fills: {} | Rebalances: {}\r",
                    elapsed_seconds / 3600,
                    (elapsed_seconds % 3600) / 60,
                    current_price,
                    total_fills,
                    total_rebalances
                );
                use std::io::{self, Write};
                io::stdout().flush().unwrap();
            }
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════════
    // FINAL REPORT
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("\n\n╔═══════════════════════════════════════╗");
    println!("║  🏁 TEST COMPLETE!                    ║");
    println!("╚═══════════════════════════════════════╝\n");
    
    if let Some(final_price) = feed.get_price(feed_ids::SOL_USD).await {
        println!("📊 RESULTS - {}", config.test_name);
        println!("════════════════════════════════════════");
        println!("  Duration:           {} hours", config.trading.test_duration_hours);
        println!("  Starting Price:     ${:.4}", initial_price);
        println!("  Ending Price:       ${:.4}", final_price);
        println!("  Price Change:       ${:.4} ({:.2}%)", 
            final_price - initial_price,
            ((final_price - initial_price) / initial_price) * 100.0
        );
        println!("  High:               ${:.4}", price_high);
        println!("  Low:                ${:.4}", price_low);
        println!("  Volatility:         ${:.4} ({:.2}%)", 
            price_high - price_low,
            ((price_high - price_low) / price_low) * 100.0
        );
        println!();
        
        engine.display_status(final_price).await;
        engine.display_performance_report().await;
        
        if config.grid.auto_rebalance {
            rebalancer.stats().display();
        }
        
        println!("\n🎯 SUMMARY");
        println!("════════════════════════════════════════");
        println!("  Total Fills:         {}", total_fills);
        println!("  Auto-Rebalances:     {}", total_rebalances);
        
        let wallet = engine.get_wallet().await;
        let final_value = wallet.total_value_usdc(final_price);
        let initial_value = config.trading.initial_usdc + (config.trading.initial_sol * initial_price);
        
        println!("  Starting Value:      ${:.2}", initial_value);
        println!("  Ending Value:        ${:.2}", final_value);
        println!("  Net Profit:          ${:.2}", final_value - initial_value);
        println!("  ROI:                 {:.2}%", wallet.roi(final_price));
        
        // Save results if enabled
        if config.monitoring.save_results {
            println!("\n💾 Saving results to: {}", config.monitoring.results_file);
            // TODO: Implement result saving
        }
    }
    
    feed.stop().await;
    println!("\n🎉 PROJECT FLASH - TEST COMPLETE! 💎\n");
}

fn print_banner() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                                                           ║");
    println!("║     🤖 CONFIG-DRIVEN GRID BOT - PROJECT FLASH 🤖         ║");
    println!("║        Customizable. Configurable. Bulletproof.          ║");
    println!("║                                                           ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");
}

fn load_config() -> BotConfig {
    // Check for config file argument
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        let config_path = &args[1];
        println!("📂 Loading config from: {}", config_path);
        
        match BotConfig::load_from_file(config_path) {
            Ok(config) => {
                println!("✅ Config loaded successfully!");
                return config;
            }
            Err(e) => {
                println!("⚠️  Failed to load config: {}", e);
                println!("   Using default overnight preset...\n");
            }
        }
    }
    
    // No file specified, use default
    println!("📋 No config file specified");
    println!("   Usage: cargo run --example rebalancing_test --release [config.toml]");
    println!("   Available presets:");
    println!("     • config/overnight.toml    (8h balanced)");
    println!("     • config/aggressive.toml   (4h high-frequency)");
    println!("     • config/conservative.toml (8h safe)");
    println!("\n   Using overnight preset...\n");
    
    BotConfig::overnight_test()
}
