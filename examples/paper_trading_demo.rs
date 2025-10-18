//! ═══════════════════════════════════════════════════════════════════════════
//! PAPER TRADING DEMO - Complete Feature Showcase
//! All-in-One: Grid Strategy + Manual Trading + Performance Analytics
//! October 14, 2025 - FIXED & WORKING
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::trading::{PythHttpFeed, feed_ids, PaperTradingEngine, OrderSide};
use tokio::time::{sleep, Duration, interval};
use log::info;
use std::io::{self, Write};

#[derive(Debug, Clone, Copy)]
enum DemoMode {
    GridStrategy,
    ManualTrading,
    QuickTest,
}

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    print_banner();
    
    // Select demo mode
    let mode = select_demo_mode();
    
    match mode {
        DemoMode::GridStrategy => run_grid_strategy().await,
        DemoMode::ManualTrading => run_manual_trading().await,
        DemoMode::QuickTest => run_quick_test().await,
    }
}

fn print_banner() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                                                           ║");
    println!("║        🎮 PAPER TRADING DEMO - Project Flash             ║");
    println!("║           Risk-Free Strategy Testing                     ║");
    println!("║                                                           ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");
}

fn select_demo_mode() -> DemoMode {
    println!("Select demo mode:\n");
    println!("  1️⃣  Grid Trading Strategy (Recommended)");
    println!("  2️⃣  Manual Trading Mode");
    println!("  3️⃣  Quick Test (60 seconds)\n");
    
    print!("Enter choice (1-3) [default: 1]: ");
    io::stdout().flush().unwrap();
    
    let mut input = String::new();
    io::stdin().read_line(&mut input).ok();
    
    match input.trim() {
        "2" => DemoMode::ManualTrading,
        "3" => DemoMode::QuickTest,
        _ => DemoMode::GridStrategy,
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// GRID STRATEGY DEMO
// ═══════════════════════════════════════════════════════════════════════════

async fn run_grid_strategy() {
    println!("\n╔═══════════════════════════════════════╗");
    println!("║  📊 GRID TRADING STRATEGY             ║");
    println!("╚═══════════════════════════════════════╝\n");
    
    // Configuration
    let initial_capital = 10000.0;
    let grid_levels = 10;
    let grid_spacing = 0.005; // 0.5% between levels
    let order_size = 1.0; // 1 SOL per order
    let duration_seconds = 120; // 2 minutes for testing
    
    info!("💰 Initial Capital: ${}", initial_capital);
    info!("📏 Grid Levels: {}", grid_levels);
    info!("📐 Grid Spacing: {}%", grid_spacing * 100.0);
    info!("📦 Order Size: {} SOL", order_size);
    info!("⏱️  Duration: {}s\n", duration_seconds);
    
    // Initialize price feed
    info!("🔌 Connecting to Pyth Network...");
    let feed = PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]);
    feed.start().await.expect("Failed to start price feed");
    
    sleep(Duration::from_secs(2)).await;
    
    // Initialize paper trading engine
    let engine = PaperTradingEngine::new(initial_capital, 0.0);
    
    // Get initial price
    let initial_price = feed.get_price(feed_ids::SOL_USD).await
        .expect("Failed to get initial price");
    
    info!("💵 Current SOL price: ${:.4}\n", initial_price);
    
    // Place initial grid
    println!("📊 Placing grid orders...\n");
    
    let half_grid = grid_levels / 2;
    
    // Buy orders below current price
    for i in 1..=half_grid {
        let price = initial_price * (1.0 - grid_spacing * i as f64);
        match engine.place_limit_order(OrderSide::Buy, price, order_size).await {
            Ok(id) => info!("  ✅ BUY  {} SOL @ ${:.4} ({})", order_size, price, id),
            Err(e) => info!("  ❌ Failed to place buy order: {}", e),
        }
    }
    
    info!("\n💡 In live mode, sell orders would be placed above ${:.4}", initial_price);
    
    // Run strategy
    println!("\n⏱️  Running strategy for {} seconds...\n", duration_seconds);
    println!("   Price updates every second");
    println!("   Status shown every 30 seconds");
    println!("   Press Ctrl+C to stop early\n");
    
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut display_counter = 0;
    
    for _i in 0..duration_seconds {
        tick_interval.tick().await;
        display_counter += 1;
        
        if let Some(current_price) = feed.get_price(feed_ids::SOL_USD).await {
            // Process price update and check for fills
            if let Ok(filled_orders) = engine.process_price_update(current_price).await {
                if !filled_orders.is_empty() {
                    println!("\n🎉 {} ORDER(S) FILLED!", filled_orders.len());
                    info!("   Maintaining grid structure...");
                }
            }
            
            // Display status every 30 seconds
            if display_counter % 30 == 0 {
                println!("\n╔═══════════════════════════════════════╗");
                println!("║   ⏰ {} seconds elapsed             ║", display_counter);
                println!("╚═══════════════════════════════════════╝");
                engine.display_status(current_price).await;
            }
        }
    }
    
    // Final summary
    println!("\n\n╔═══════════════════════════════════════╗");
    println!("║  ✅ STRATEGY TEST COMPLETE            ║");
    println!("╚═══════════════════════════════════════╝\n");
    
    if let Some(final_price) = feed.get_price(feed_ids::SOL_USD).await {
        engine.display_status(final_price).await;
        engine.display_performance_report().await;
    }
    
    feed.stop().await;
}

// ═══════════════════════════════════════════════════════════════════════════
// MANUAL TRADING DEMO
// ═══════════════════════════════════════════════════════════════════════════

async fn run_manual_trading() {
    println!("\n╔═══════════════════════════════════════╗");
    println!("║  🎯 MANUAL TRADING MODE               ║");
    println!("╚═══════════════════════════════════════╝\n");
    
    println!("Commands:");
    println!("  buy <price> <size>  - Place buy order");
    println!("  sell <price> <size> - Place sell order");
    println!("  status              - Show current status");
    println!("  orders              - Show open orders");
    println!("  cancel <id>         - Cancel order");
    println!("  exit                - Exit demo\n");
    
    // Initialize
    let feed = PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]);
    feed.start().await.expect("Failed to start price feed");
    sleep(Duration::from_secs(2)).await;
    
    let engine = PaperTradingEngine::new(10000.0, 10.0);
    
    // Command loop
    loop {
        print!("\n> ");
        io::stdout().flush().unwrap();
        
        let mut input = String::new();
        io::stdin().read_line(&mut input).ok();
        
        let parts: Vec<&str> = input.trim().split_whitespace().collect();
        if parts.is_empty() {
            continue;
        }
        
        // Process price updates before each command
        if let Some(current_price) = feed.get_price(feed_ids::SOL_USD).await {
            let _ = engine.process_price_update(current_price).await;
        }
        
        match parts[0].to_lowercase().as_str() {
            "buy" if parts.len() == 3 => {
                if let (Ok(price), Ok(size)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                    match engine.place_limit_order(OrderSide::Buy, price, size).await {
                        Ok(id) => println!("✅ Buy order placed: {}", id),
                        Err(e) => println!("❌ Error: {}", e),
                    }
                } else {
                    println!("❌ Invalid price or size");
                }
            }
            "sell" if parts.len() == 3 => {
                if let (Ok(price), Ok(size)) = (parts[1].parse::<f64>(), parts[2].parse::<f64>()) {
                    match engine.place_limit_order(OrderSide::Sell, price, size).await {
                        Ok(id) => println!("✅ Sell order placed: {}", id),
                        Err(e) => println!("❌ Error: {}", e),
                    }
                } else {
                    println!("❌ Invalid price or size");
                }
            }
            "status" => {
                if let Some(price) = feed.get_price(feed_ids::SOL_USD).await {
                    engine.display_status(price).await;
                }
            }
            "orders" => {
                let orders = engine.get_open_orders().await;
                println!("\n📋 Open Orders: {}", orders.len());
                for order in orders {
                    println!("  {} | {} {} SOL @ ${:.4}",
                        order.id,
                        match order.side { OrderSide::Buy => "BUY ", OrderSide::Sell => "SELL" },
                        order.size,
                        order.price
                    );
                }
            }
            "cancel" if parts.len() == 2 => {
                match engine.cancel_order(parts[1]).await {
                    Ok(_) => println!("✅ Order cancelled"),
                    Err(e) => println!("❌ Error: {}", e),
                }
            }
            "exit" => {
                println!("\n👋 Exiting...");
                feed.stop().await;
                break;
            }
            _ => println!("❓ Unknown command. Try: buy, sell, status, orders, cancel, exit"),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// QUICK TEST DEMO
// ═══════════════════════════════════════════════════════════════════════════

async fn run_quick_test() {
    println!("\n╔═══════════════════════════════════════╗");
    println!("║  ⚡ QUICK TEST (60 seconds)           ║");
    println!("╚═══════════════════════════════════════╝\n");
    
    let feed = PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]);
    feed.start().await.expect("Failed to start price feed");
    sleep(Duration::from_secs(2)).await;
    
    let engine = PaperTradingEngine::new(10000.0, 0.0);
    
    let initial_price = feed.get_price(feed_ids::SOL_USD).await.unwrap();
    info!("💵 Current price: ${:.4}", initial_price);
    
    // Place a few test orders
    println!("\n📝 Placing test orders...\n");
    for i in 1..=3 {
        let price = initial_price * (1.0 - 0.01 * i as f64);
        match engine.place_limit_order(OrderSide::Buy, price, 1.0).await {
            Ok(id) => info!("  ✅ Order placed: {} @ ${:.4}", id, price),
            Err(e) => info!("  ❌ Failed: {}", e),
        }
    }
    
    println!("\n⏱️  Running for 60 seconds...\n");
    
    for i in 1..=60 {
        sleep(Duration::from_secs(1)).await;
        
        if let Some(price) = feed.get_price(feed_ids::SOL_USD).await {
            if let Ok(filled) = engine.process_price_update(price).await {
                if !filled.is_empty() {
                    println!("🎉 {} order(s) filled!", filled.len());
                }
            }
            
            if i % 15 == 0 {
                println!("\n⏰ {} seconds", i);
                engine.display_status(price).await;
            }
        }
    }
    
    println!("\n✅ Quick test complete!");
    feed.stop().await;
}
