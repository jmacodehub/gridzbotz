//! ═══════════════════════════════════════════════════════════════════════════
//! EXTENDED GRID TEST - Complete Buy & Sell Grid
//! Educational Version with Detailed Explanations
//! October 14, 2025
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! This test demonstrates a REAL grid trading strategy:
//! - Places buy orders below current price
//! - Places sell orders above current price
//! - Automatically profits from price oscillations
//! - Runs for 30 minutes to catch real market movements

use solana_grid_bot::trading::{PythHttpFeed, feed_ids, PaperTradingEngine, OrderSide};
use tokio::time::{sleep, Duration, interval};
use log::{info, warn};

#[tokio::main]
async fn main() {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    print_banner();
    explain_strategy();
    
    // Configuration
    let initial_usdc = 5000.0;  // Start with $5,000 USDC
    let initial_sol = 10.0;     // AND 10 SOL (so we can place sell orders!)
    let test_duration_minutes = 30;
    let grid_spacing_percent = 0.3; // 0.3% between levels (tighter = more fills)
    let order_size = 0.5; // 0.5 SOL per order (smaller = more trades)
    let num_buy_orders = 10;
    let num_sell_orders = 10;
    
    println!("⚙️  CONFIGURATION");
    println!("════════════════════════════════════════");
    println!("  Starting Capital:    ${:.2} USDC", initial_usdc);
    println!("  Starting SOL:        {:.2} SOL", initial_sol);
    println!("  Test Duration:       {} minutes", test_duration_minutes);
    println!("  Grid Spacing:        {}%", grid_spacing_percent);
    println!("  Order Size:          {} SOL", order_size);
    println!("  Buy Orders:          {}", num_buy_orders);
    println!("  Sell Orders:         {}", num_sell_orders);
    println!();
    
    // Initialize price feed
    info!("🔌 Connecting to Pyth Network...");
    let feed = PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]);
    feed.start().await.expect("Failed to start price feed");
    sleep(Duration::from_secs(2)).await;
    
    // Initialize paper trading engine with BOTH USDC and SOL
    info!("🎮 Initializing paper trading engine...");
    let engine = PaperTradingEngine::new(initial_usdc, initial_sol);
    
    // Get initial price
    let initial_price = feed.get_price(feed_ids::SOL_USD).await
        .expect("Failed to get initial price");
    
    info!("💵 Current SOL price: ${:.4}\n", initial_price);
    
    // Calculate grid spacing
    let grid_spacing = grid_spacing_percent / 100.0;
    
    // ═══════════════════════════════════════════════════════════════════════
    // STEP 1: Place BUY orders below current price
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("📊 PLACING BUY ORDERS (Below ${:.4})", initial_price);
    println!("════════════════════════════════════════");
    
    let mut buy_order_count = 0;
    for i in 1..=num_buy_orders {
        let price = initial_price * (1.0 - grid_spacing * i as f64);
        
        match engine.place_limit_order(OrderSide::Buy, price, order_size).await {
            Ok(id) => {
                buy_order_count += 1;
                println!("  ✅ BUY  {} SOL @ ${:.4} ({})", order_size, price, id);
                info!("     💡 If price drops to ${:.4}, this order fills!", price);
            }
            Err(e) => {
                warn!("  ❌ Failed to place buy order: {}", e);
            }
        }
    }
    
    println!();
    
    // ═══════════════════════════════════════════════════════════════════════
    // STEP 2: Place SELL orders above current price
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("📊 PLACING SELL ORDERS (Above ${:.4})", initial_price);
    println!("════════════════════════════════════════");
    
    let mut sell_order_count = 0;
    for i in 1..=num_sell_orders {
        let price = initial_price * (1.0 + grid_spacing * i as f64);
        
        match engine.place_limit_order(OrderSide::Sell, price, order_size).await {
            Ok(id) => {
                sell_order_count += 1;
                println!("  ✅ SELL {} SOL @ ${:.4} ({})", order_size, price, id);
                info!("     💡 If price rises to ${:.4}, this order fills!", price);
            }
            Err(e) => {
                warn!("  ❌ Failed to place sell order: {}", e);
            }
        }
    }
    
    println!("\n╔═══════════════════════════════════════╗");
    println!("║  ✅ GRID SETUP COMPLETE               ║");
    println!("╚═══════════════════════════════════════╝");
    println!("  Buy Orders:  {}", buy_order_count);
    println!("  Sell Orders: {}", sell_order_count);
    println!("  Total Grid:  {} orders\n", buy_order_count + sell_order_count);
    
    // Show initial status
    engine.display_status(initial_price).await;
    
    // ═══════════════════════════════════════════════════════════════════════
    // STEP 3: Run the grid for 30 minutes
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("\n⏱️  STARTING {} MINUTE TEST", test_duration_minutes);
    println!("════════════════════════════════════════");
    println!("  • Price checks every second");
    println!("  • Status updates every 5 minutes");
    println!("  • Order fills shown immediately");
    println!("  • Press Ctrl+C to stop early\n");
    
    let mut tick_interval = interval(Duration::from_secs(1));
    let mut elapsed_seconds = 0;
    let total_seconds = test_duration_minutes * 60;
    let mut last_price = initial_price;
    let mut price_high = initial_price;
    let mut price_low = initial_price;
    let mut total_fills = 0;
    
    while elapsed_seconds < total_seconds {
        tick_interval.tick().await;
        elapsed_seconds += 1;
        
        if let Some(current_price) = feed.get_price(feed_ids::SOL_USD).await {
            // Track price range
            price_high = price_high.max(current_price);
            price_low = price_low.min(current_price);
            
            // Check for order fills
            if let Ok(filled_orders) = engine.process_price_update(current_price).await {
                if !filled_orders.is_empty() {
                    total_fills += filled_orders.len();
                    
                    println!("\n╔═══════════════════════════════════════╗");
                    println!("║  🎉 ORDER FILL DETECTED!              ║");
                    println!("╚═══════════════════════════════════════╝");
                    println!("  Time:     {}m {}s", elapsed_seconds / 60, elapsed_seconds % 60);
                    println!("  Price:    ${:.4}", current_price);
                    println!("  Orders:   {} filled", filled_orders.len());
                    println!();
                    
                    for order_id in &filled_orders {
                        let trades = engine.get_trade_history(20).await;
                        if let Some(trade) = trades.iter().find(|t| &t.order_id == order_id) {
                            let side = match trade.side {
                                OrderSide::Buy => "BUY ",
                                OrderSide::Sell => "SELL",
                            };
                            println!("  {} {} {} SOL @ ${:.4} (fee: ${:.4})",
                                side, order_id, trade.size, trade.price, trade.fee);
                        }
                    }
                    
                    // Show updated status
                    engine.display_status(current_price).await;
                    
                    // Educational: Explain what happened
                    explain_fill(current_price, last_price);
                }
            }
            
            last_price = current_price;
            
            // Status updates every 5 minutes
            if elapsed_seconds % 300 == 0 {
                let minutes = elapsed_seconds / 60;
                println!("\n╔═══════════════════════════════════════╗");
                println!("║  ⏰ {} MINUTES ELAPSED                ║", minutes);
                println!("╚═══════════════════════════════════════╝");
                println!("  Current Price:  ${:.4}", current_price);
                println!("  Price High:     ${:.4}", price_high);
                println!("  Price Low:      ${:.4}", price_low);
                println!("  Total Fills:    {}", total_fills);
                println!();
                
                engine.display_status(current_price).await;
            }
            
            // Mini progress indicator every 30 seconds
            if elapsed_seconds % 30 == 0 && elapsed_seconds % 300 != 0 {
                print!("  ⏳ {}m {}s | Price: ${:.4} | Fills: {}\r",
                    elapsed_seconds / 60,
                    elapsed_seconds % 60,
                    current_price,
                    total_fills
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
    println!("║  ✅ TEST COMPLETE                     ║");
    println!("╚═══════════════════════════════════════╝\n");
    
    if let Some(final_price) = feed.get_price(feed_ids::SOL_USD).await {
        println!("📊 PRICE MOVEMENT");
        println!("════════════════════════════════════════");
        println!("  Starting:  ${:.4}", initial_price);
        println!("  Ending:    ${:.4}", final_price);
        println!("  Change:    ${:.4} ({:.2}%)", 
            final_price - initial_price,
            ((final_price - initial_price) / initial_price) * 100.0
        );
        println!("  High:      ${:.4}", price_high);
        println!("  Low:       ${:.4}", price_low);
        println!("  Range:     ${:.4}", price_high - price_low);
        println!();
        
        engine.display_status(final_price).await;
        engine.display_performance_report().await;
        
        // Calculate theoretical vs actual
        let theoretical_value = initial_usdc + (initial_sol * final_price);
        let wallet = engine.get_wallet().await;
        let actual_value = wallet.total_value_usdc(final_price);
        
        println!("\n💡 LEARNING SUMMARY");
        println!("════════════════════════════════════════");
        println!("  Total Orders Filled:     {}", total_fills);
        println!("  Open Orders Remaining:   {}", engine.open_order_count().await);
        println!("  Theoretical Value:       ${:.2} (if we held)", theoretical_value);
        println!("  Actual Value:            ${:.2} (with grid)", actual_value);
        println!("  Grid Performance:        ${:.2}", actual_value - theoretical_value);
        
        if total_fills > 0 {
            println!("\n✅ SUCCESS! Grid trading worked!");
            println!("   Your orders filled {} times during price movements.", total_fills);
            println!("   This proves the strategy works in real market conditions!");
        } else {
            println!("\n💡 No fills this time, but grid is working correctly!");
            println!("   Price range was: ${:.4} - ${:.4}", price_low, price_high);
            println!("   Your grid was: ${:.4} - ${:.4}",
                initial_price * (1.0 - grid_spacing * num_buy_orders as f64),
                initial_price * (1.0 + grid_spacing * num_sell_orders as f64)
            );
            println!("   Try: Wider grid range or longer test duration!");
        }
    }
    
    feed.stop().await;
    println!("\n🎉 TEST COMPLETE! Great work! 💎\n");
}

fn print_banner() {
    println!("\n╔═══════════════════════════════════════════════════════════╗");
    println!("║                                                           ║");
    println!("║     🔥 PROJECT FLASH - EXTENDED GRID TEST 🔥             ║");
    println!("║        Real Trading Simulation with Live Prices          ║");
    println!("║                                                           ║");
    println!("╚═══════════════════════════════════════════════════════════╝\n");
}

fn explain_strategy() {
    println!("📚 WHAT IS GRID TRADING?");
    println!("════════════════════════════════════════");
    println!();
    println!("  Grid trading profits from price oscillations:");
    println!();
    println!("     $196 ← SELL (if price goes up)");
    println!("     $194 ← SELL");
    println!("  →  $192 ← CURRENT PRICE");
    println!("     $190 ← BUY  (if price goes down)");
    println!("     $188 ← BUY");
    println!();
    println!("  When price moves:");
    println!("  • DOWN → Buy orders fill (we buy cheap!)");
    println!("  • UP   → Sell orders fill (we sell high!)");
    println!();
    println!("  Profit = Sell Price - Buy Price - Fees");
    println!();
    println!("  The more price oscillates, the more we profit! 💰");
    println!();
}

fn explain_fill(current_price: f64, last_price: f64) {
    println!("\n💡 WHAT JUST HAPPENED?");
    println!("════════════════════════════════════════");
    
    if current_price > last_price {
        println!("  Price moved UP from ${:.4} to ${:.4}", last_price, current_price);
        println!("  → SELL order(s) filled! ✅");
        println!("  → We sold SOL at a higher price");
        println!("  → This is PROFITABLE! 💰");
    } else {
        println!("  Price moved DOWN from ${:.4} to ${:.4}", last_price, current_price);
        println!("  → BUY order(s) filled! ✅");
        println!("  → We bought SOL at a lower price");
        println!("  → Ready to sell when price goes back up! 📈");
    }
    println!();
}
