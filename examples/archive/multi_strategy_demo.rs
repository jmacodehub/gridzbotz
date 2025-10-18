//! 🎯 Multi-Strategy Demo
//! 
//! Demonstrates all 3 strategies working together

use solana_grid_bot::strategies::*;
use solana_grid_bot::trading::PythPriceFeed;
use anyhow::Result;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> Result<()> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║        🎯 MULTI-STRATEGY TRADING ENGINE DEMO 🎯               ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    let mut manager = StrategyManager::new();
    
    println!("📈 Loading trading strategies...\n");
    manager.add_strategy(Box::new(MomentumStrategy::new()));
    manager.add_strategy(Box::new(MeanReversionStrategy::new()));
    manager.add_strategy(Box::new(RSIStrategy::new()));
    manager.add_strategy(Box::new(ArbitrageStrategy::new())); 

    println!("✅ Loaded {} strategies\n", manager.strategy_count());
    manager.set_consensus_mode(ConsensusMode::WeightedAverage);
    println!("🎲 Consensus mode: Weighted Average\n");
    
    println!("📡 Connecting to Pyth price feed...");
    let price_feed = PythPriceFeed::new()
        .map_err(|e| anyhow::anyhow!("Failed to create price feed: {}", e))?;
    
    println!("⏳ Waiting for initial price data...\n");
    sleep(Duration::from_secs(2)).await;
    
    println!("╔═══════════════════════════════════════════════════════════════╗");
    println!("║              📊 LIVE STRATEGY ANALYSIS                        ║");
    println!("╚═══════════════════════════════════════════════════════════════╝\n");
    
    for cycle in 1..=10 {
        let price = price_feed.latest_price().await;
        
        if price > 0.0 {
            let timestamp = chrono::Utc::now().timestamp();
            
            println!("─────────────────────────────────────────────────────────────");
            println!("📊 Cycle {}/10 | SOL/USD: ${:.2}", cycle, price);
            println!("─────────────────────────────────────────────────────────────\n");
            
            match manager.analyze_all(price, timestamp).await {
                Ok(signals) => {
                    for (strategy_name, signal) in &signals {
                        println!("🎯 {}", strategy_name);
                        println!("   {}", signal.display());
                        println!();
                    }
                    
                    println!("╔═══════════════════════════════════════════════╗");
                    println!("║  ALL STRATEGIES ANALYZED                      ║");
                    println!("╚═══════════════════════════════════════════════╝\n");
                }
                Err(e) => eprintln!("❌ Analysis error: {}", e),
            }
        } else {
            println!("⏳ Waiting for price data...");
        }
        
        sleep(Duration::from_secs(3)).await;
    }
    
    println!("\n╔═══════════════════════════════════════════════════════════════╗");
    println!("║              📊 FINAL PERFORMANCE REPORT                      ║");
    println!("╚═══════════════════════════════════════════════════════════════╝");
    
    manager.display_stats();
    
    println!("\n✅ Demo completed successfully!\n");
    
    Ok(())
}
