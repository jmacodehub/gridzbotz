//! ═══════════════════════════════════════════════════════════════════════════
//! CONFIG-DRIVEN TEST RUNNER - PROJECT FLASH
//! Loads all settings from TOML files
//! October 14, 2025
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::config::Config;
use solana_grid_bot::trading::{PythHttpFeed, feed_ids, PaperTradingEngine, OrderSide};
use solana_grid_bot::strategies::{GridRebalancer, GridRebalancerConfig};
use tokio::time::{sleep, Duration, interval};
use log::{info, warn};
use std::env;

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    print_banner();
    
    // Load configuration
    let config = load_config();
    config.display_summary();
    
    println!("🚀 Press Enter to start, or Ctrl+C to cancel...");
    let mut input = String::new();
    std::io::stdin().read_line(&mut input).ok();
    
    // Initialize systems
    info!("🔌 Connecting to Pyth Network...");
    let feed = PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]);
    feed.start().await.expect("Failed to start price feed");
    sleep(Duration::from_secs(2)).await;
    
    info!("🎮 Initializing paper trading engine...");
    let engine = PaperTradingEngine::new(
        config.paper_trading.initial_usdc,
        config.paper_trading.initial_sol
    );
    
    info!("🤖 Initializing grid rebalancer...");
    let rebalancer_config = GridRebalancerConfig {
        grid_spacing: config.trading.grid_spacing_percent / 100.0,
        order_size: config.trading.min_order_size,
        min_usdc_balance: config.trading.min_usdc_reserve,
        min_sol_balance: config.trading.min_sol_reserve,
        enabled: config.trading.enable_auto_rebalance,
    };
    let mut rebalancer = GridRebalancer::new(rebalancer_config);
    
    let initial_price = feed.get_price(feed_ids::SOL_USD).await
        .expect("Failed to get initial price");
    
    info!("💵 Current SOL price: ${:.4}\n", initial_price);
    
    // Place initial grid
    println!("📊 PLACING GRID");
    println!("════════════════════════════════════════");
    
    let num_orders = config.trading.grid_levels / 2;
    let spacing = config.trading.grid_spacing_percent / 100.0;
    
    let mut buy_count = 0;
    for i in 1..=num_orders {
        let price = initial_price * (1.0 - spacing * i as f64);
        if let Ok(id) = engine.place_limit_order(OrderSide::Buy, price, config.trading.min_order_size).await {
            buy_count += 1;
            if i <= 3 {
                println!("  ✅ BUY  {} SOL @ ${:.4} ({})", config.trading.min_order_size, price, id);
            }
        }
    }
    if num_orders > 3 {
        println!("  ... and {} more buy orders", num_orders - 3);
    }
    
    println!();
    
    let mut sell_count = 0;
    for i in 1..=num_orders {
        let price = initial_price * (1.0 + spacing * i as f64);
        if let Ok(id) = engine.place_limit_order(OrderSide::Sell, price, config.trading.min_order_size).await {
            sell_count += 1;
            if i <= 3 {
                println!("  ✅ SELL {} SOL @ ${:.4} ({})", config.trading.min_order_size, price, id);
            }
        }
    }
    if num_orders > 3 {
        println!("  ... and {} more sell orders", num_orders - 3);
    }
    
    println!("\n╔═══════════════════════════════════════╗");
    println!("║  ✅ GRID READY - {} ORDERS           ║", buy_count + sell_count);
    println!("╚═══════════════════════════════════════╝");
    println!("  Buy Orders:    {}", buy_count);
    println!("  Sell Orders:   {}", sell_count);
    println!("  Rebalancing:   {}\n", if config.trading.enable_auto_rebalance { "🤖 AUTO" } else { "❌ OFF" });
    
    engine.display_status(initial_price).await;
    
    // Main trading loop
    println!("\n⏱️  STARTING {} HOUR TEST", config.paper_trading.test_duration_hours);
    println!("════════════════════════════════════════");
    println!("  • Config:      Loaded from file");
    println!("  • Rebalancing: {}", if config.trading.enable_auto_rebalance { "✅" } else { "❌" });
    println!("  • Press Ctrl+C to stop\n");
    
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut elapsed_seconds = 0;
    let total_seconds = config.paper_trading.test_duration_hours * 3600;
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
            
            if let Ok(filled_orders) = engine.process_price_update(current_price).await {
                if !filled_orders.is_empty() {
                    total_fills += filled_orders.len();
                    
                    println!("\n╔═══════════════════════════════════════╗");
                    println!("║  🎉 {} ORDER(S) FILLED!               ║", filled_orders.len());
                    println!("╚═══════════════════════════════════════╝");
                    println!("  Time:  {}h {}m", elapsed_seconds / 3600, (elapsed_seconds % 3600) / 60);
                    println!("  Price: ${:.4}", current_price);
                    
                    if config.trading.enable_auto_rebalance {
                        println!("\n🤖 AUTO-REBALANCING...");
                        match rebalancer.rebalance_after_fills(&filled_orders, &engine, current_price).await {
                            Ok(new_orders) => {
                                if !new_orders.is_empty() {
                                    total_rebalances += new_orders.len();
                                    println!("  ✅ Placed {} new order(s)", new_orders.len());
                                }
                            }
                            Err(e) => warn!("  ⚠️  Rebalancing error: {}", e),
                        }
                    }
                    
                    engine.display_status(current_price).await;
                }
            }
            
            // Status updates
            if elapsed_seconds % (config.paper_trading.status_interval_sec as usize) == 0 {
                println!("\n╔═══════════════════════════════════════╗");
                println!("║  ⏰ {}h {}m ELAPSED                   ║", 
                    elapsed_seconds / 3600, (elapsed_seconds % 3600) / 60);
                println!("╚═══════════════════════════════════════╝");
                println!("  Price:     ${:.4}", current_price);
                println!("  Range:     ${:.4} - ${:.4}", price_low, price_high);
                println!("  Fills:     {}", total_fills);
                println!("  Rebalances: {}\n", total_rebalances);
                
                engine.display_status(current_price).await;
            }
        }
    }
    
    // Final report
    println!("\n\n╔═══════════════════════════════════════╗");
    println!("║  🏁 TEST COMPLETE!                    ║");
    println!("╚═══════════════════════════════════════╝\n");
    
    if let Some(final_price) = feed.get_price(feed_ids::SOL_USD).await {
        println!("📊 FINAL RESULTS - {}", config.bot.name);
        println!("════════════════════════════════════════");
        println!("  Duration:      {} hours", config.paper_trading.test_duration_hours);
        println!("  Total Fills:   {}", total_fills);
        println!("  Rebalances:    {}", total_rebalances);
        
        engine.display_status(final_price).await;
        engine.display_performance_report().await;
    }
    
    feed.stop().await;
    println!("\n🎉 PROJECT FLASH - CONFIG TEST COMPLETE! 💎\n");
}

fn print_banner() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                                                           ║");
    println!("║        🤖 CONFIG-DRIVEN GRID BOT - PROJECT FLASH 🤖      ║");
    println!("║              Bulletproof. Configurable. Scalable.        ║");
    println!("║                                                           ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");
}

fn load_config() -> Config {
    let args: Vec<String> = env::args().collect();
    
    if args.len() > 1 {
        let config_path = &args[1];
        match Config::from_file(config_path) {
            Ok(config) => return config,
            Err(e) => {
                println!("❌ Failed to load config: {}", e);
                println!("   Using default overnight preset...\n");
            }
        }
    } else {
        println!("💡 Usage: cargo run --example config_test --release <config.toml>");
        println!("   Available configs:");
        println!("     • config/overnight.toml");
        println!("     • config/aggressive.toml");
        println!("     • config/multi_strategy.toml");
        println!("\n   Using default overnight preset...\n");
    }
    
    Config::overnight_test()
}
