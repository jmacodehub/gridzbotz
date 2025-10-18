//! 🚀 Complete Trading System Integration
//! 
//! Demonstrates:
//! - Real-time Pyth price feeds
//! - Dynamic grid repositioning
//! - DEX order placement (simulation)
//! - Position sizing with Kelly Criterion
//! - Stop-loss & take-profit
//! - Circuit breaker protection

use solana_grid_bot::{Config, trading::{GridBot, PythPriceFeed}, risk::*};
use std::{error::Error, sync::Arc, time::Duration};
use log::{info, warn};
use tokio::time::sleep;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    // Initialize logging
    env_logger::Builder::from_default_env()
        .filter_level(log::LevelFilter::Info)
        .init();
    
    print_banner();
    
    // ═══════════════════════════════════════════════════════════════════════
    // Phase 1: Load Configuration & Initialize Systems
    // ═══════════════════════════════════════════════════════════════════════
    
    info!("🔧 Initializing trading system...");
    let config = Config::load()?;
    config.display_compact();
    
    // Initialize core trading components
    let mut grid_bot = GridBot::new(config.clone())?;
    grid_bot.initialize().await?;
    
    // Initialize price feed
    let price_feed = Arc::new(PythPriceFeed::new()?);
    price_feed.start().await?;
    
    // Initialize risk management
    let position_sizer = PositionSizer::new(&config);
    let mut stop_loss = StopLossManager::new(&config);
    let mut circuit_breaker = CircuitBreaker::new(&config);
    
    info!("✅ All systems initialized\n");
    sleep(Duration::from_millis(1000)).await;
    
    // ═══════════════════════════════════════════════════════════════════════
    // Phase 2: Trading State Management
    // ═══════════════════════════════════════════════════════════════════════
    
    #[derive(Debug)]
    struct TradingState {
        in_position: bool,
        entry_price: f64,
        position_size: f64,
        total_pnl: f64,
        win_count: u32,
        loss_count: u32,
    }
    
    let mut state = TradingState {
        in_position: false,
        entry_price: 0.0,
        position_size: 0.0,
        total_pnl: 0.0,
        win_count: 0,
        loss_count: 0,
    };
    
    // ═══════════════════════════════════════════════════════════════════════
    // Phase 3: Main Trading Loop (50 cycles demo)
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("🔥 STARTING INTEGRATED TRADING SYSTEM (50 cycles)\n");
    println!("{}", "─".repeat(80));
    
    for cycle in 1..=50 {
        // Check circuit breaker
        if !circuit_breaker.is_trading_allowed() {
            warn!("⏸️  Trading paused by circuit breaker");
            sleep(Duration::from_millis(500)).await;
            continue;
        }
        
        // Get current price
        let current_price = price_feed.latest_price().await;
        
        // Check if grid needs repositioning
        let should_reposition = grid_bot.should_reposition(current_price, state.entry_price);
        
        println!("\n═══════════════════════════════════════════════════════════════════════");
        println!("📊 Cycle #{:>2}/50 | SOL/USD: ${:.4}", cycle, current_price);
        println!("═══════════════════════════════════════════════════════════════════════");
        
        // ───────────────────────────────────────────────────────────────────
        // Position Management Logic
        // ───────────────────────────────────────────────────────────────────
        
        if state.in_position {
            // We have an open position - check exit conditions
            println!("📍 Current Position:");
            println!("   Entry:    ${:.4}", state.entry_price);
            println!("   Current:  ${:.4}", current_price);
            println!("   Size:     {:.4} SOL", state.position_size);
            
            let pnl_pct = ((current_price - state.entry_price) / state.entry_price) * 100.0;
            let pnl_usd = (current_price - state.entry_price) * state.position_size;
            
            println!("   P&L:      {:+.2}% (${:+.2})", pnl_pct, pnl_usd);
            
            // Check stop-loss
            if stop_loss.should_stop_loss(state.entry_price, current_price) {
                println!("\n🛑 STOP-LOSS TRIGGERED - Closing position");
                state.in_position = false;
                state.total_pnl += pnl_usd;
                state.loss_count += 1;
                
                // Record trade for circuit breaker
                circuit_breaker.record_trade(pnl_pct);
                
                println!("   Result: LOSS of ${:.2}", pnl_usd.abs());
                println!("   Total P&L: ${:.2}", state.total_pnl);
            }
            // Check take-profit
            else if stop_loss.should_take_profit(state.entry_price, current_price) {
                println!("\n🎯 TAKE-PROFIT HIT - Closing position");
                state.in_position = false;
                state.total_pnl += pnl_usd;
                state.win_count += 1;
                
                // Record trade for circuit breaker
                circuit_breaker.record_trade(pnl_pct);
                
                println!("   Result: PROFIT of ${:.2}", pnl_usd);
                println!("   Total P&L: ${:.2}", state.total_pnl);
            } else {
                println!("   Status: ✅ Holding position");
            }
            
        } else {
            // No position - look for entry opportunities
            println!("💤 No Position - Analyzing market...");
            
            // Check if grid needs repositioning (entry signal)
            if should_reposition {
                println!("\n🎯 ENTRY SIGNAL DETECTED!");
                
                // Calculate position size
                let win_rate = if (state.win_count + state.loss_count) > 0 {
                    state.win_count as f64 / (state.win_count + state.loss_count) as f64
                } else {
                    0.55 // Start with slight edge assumption
                };
                
                let volatility = 0.02; // 2% volatility estimate (will enhance later)
                let size = position_sizer.calculate_size(current_price, volatility, win_rate);
                
                // Validate size
                if position_sizer.validate_size(size, current_price).is_ok() {
                    println!("\n📝 Opening new position:");
                    println!("   Entry Price: ${:.4}", current_price);
                    println!("   Position Size: {:.4} SOL", size);
                    println!("   Position Value: ${:.2}", size * current_price);
                    
                    // Reposition grid
                    grid_bot.reposition_grid(current_price, 0.0).await?;
                    
                    // Open position
                    state.in_position = true;
                    state.entry_price = current_price;
                    state.position_size = size;
                    
                    // Reset stop-loss for new position
                    stop_loss.reset(current_price);
                    
                    println!("   ✅ Position opened successfully!");
                } else {
                    warn!("⚠️  Position size validation failed - skipping entry");
                }
            } else {
                println!("   Status: ⏳ Waiting for setup...");
            }
        }
        
        // ───────────────────────────────────────────────────────────────────
        // Statistics & Progress
        // ───────────────────────────────────────────────────────────────────
        
        if cycle % 10 == 0 || cycle == 50 {
            println!("\n{}", "─".repeat(80));
            println!("📈 Performance Summary:");
            println!("   Total P&L:        ${:.2}", state.total_pnl);
            println!("   Win Rate:         {:.1}% ({} wins / {} losses)", 
                     if (state.win_count + state.loss_count) > 0 {
                         (state.win_count as f64 / (state.win_count + state.loss_count) as f64) * 100.0
                     } else { 0.0 },
                     state.win_count, state.loss_count);
            println!("   Feed Health:      {}", 
                     if price_feed.is_healthy() { "✅ Healthy" } else { "⚠️  Degraded" });
            println!("   RPC Success:      {:.1}%", price_feed.success_rate());
            println!("{}", "─".repeat(80));
        }
        
        // Delay between cycles
        sleep(Duration::from_millis(100)).await;
    }
    
    // ═══════════════════════════════════════════════════════════════════════
    // Phase 4: Final Summary
    // ═══════════════════════════════════════════════════════════════════════
    
    println!("\n{}", "═".repeat(80));
    println!("  🏁 TRADING SESSION COMPLETED");
    println!("{}", "═".repeat(80));
    
    println!("\n💰 Final Results:");
    println!("   Total P&L:           ${:.2}", state.total_pnl);
    println!("   Total Trades:        {}", state.win_count + state.loss_count);
    println!("   Winning Trades:      {}", state.win_count);
    println!("   Losing Trades:       {}", state.loss_count);
    
    if (state.win_count + state.loss_count) > 0 {
        let win_rate = (state.win_count as f64 / (state.win_count + state.loss_count) as f64) * 100.0;
        println!("   Win Rate:            {:.1}%", win_rate);
        println!("   Avg Profit/Trade:    ${:.2}", state.total_pnl / (state.win_count + state.loss_count) as f64);
    }
    
    println!("\n📊 System Health:");
    println!("   Price Feed:          {}", 
             if price_feed.is_healthy() { "✅ Healthy" } else { "⚠️  Degraded" });
    println!("   RPC Success Rate:    {:.1}%", price_feed.success_rate());
    println!("   Circuit Breaker:     {}", 
             if circuit_breaker.is_trading_allowed() { "✅ Active" } else { "⚠️  Tripped" });
    
    println!("\n{}", "═".repeat(80));
    println!("\n🎉 Demo completed successfully!\n");
    
    Ok(())
}

fn print_banner() {
    println!("\n{}", "═".repeat(80));
    println!("     🚀 SOLANA GRID BOT - INTEGRATED TRADING SYSTEM");
    println!("     Phase 2 ✅ DEX Ready | Phase 3 ✅ Risk Management");
    println!("{}\n", "═".repeat(80));
}
