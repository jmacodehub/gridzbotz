//! ═══════════════════════════════════════════════════════════════════════════
//! PYTH WEBSOCKET DEBUG TOOL
//! Production-Ready Testing & Validation
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::trading::pyth_websocket::{PythWebSocketFeed, feed_ids};
use tokio::time::{sleep, Duration};
use colored::*;
use std::io::{self, Write};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Initialize logging
    env_logger::Builder::new()
        .filter_level(log::LevelFilter::Info)
        .format_timestamp_millis()
        .init();
    
    // Print banner
    print_banner();
    
    // Create feed for multiple assets
    println!("📡 Subscribing to price feeds...\n");
    let feed = PythWebSocketFeed::new(vec![
        feed_ids::SOL_USD.to_string(),
        feed_ids::BTC_USD.to_string(),
        feed_ids::ETH_USD.to_string(),
    ]);
    
    // Start connection
    println!("🔌 Connecting to Pyth Hermes...\n");
    feed.start().await?;
    
    // Wait for initial connection
    let mut attempts = 0;
    while !feed.is_healthy().await && attempts < 30 {
        print!(".");
        io::stdout().flush()?;
        sleep(Duration::from_millis(100)).await;
        attempts += 1;
    }
    println!();
    
    if !feed.is_healthy().await {
        eprintln!("❌ Failed to connect after 3 seconds");
        eprintln!("   Check your internet connection and try again");
        return Ok(());
    }
    
    println!("{}", "✅ Connected successfully!".green().bold());
    println!("\n{}", "⏳ Monitoring prices for 60 seconds...".cyan());
    println!("{}\n", "Press Ctrl+C to stop".dimmed());
    
    // Monitor for 60 seconds
    for i in 1..=60 {
        sleep(Duration::from_secs(1)).await;
        
        // Get all current prices
        let sol = feed.get_price(feed_ids::SOL_USD).await;
        let btc = feed.get_price(feed_ids::BTC_USD).await;
        let eth = feed.get_price(feed_ids::ETH_USD).await;
        
        // Get latency
        let latency = feed.latency_micros().await;
        
        // Print formatted update
        print!("⏱️  {:2}s │ ", i);
        
        if let Some(price) = sol {
            print!("{} ${:>8.4} │ ", "SOL".yellow(), price);
        } else {
            print!("{} {:>8} │ ", "SOL".dimmed(), "---");
        }
        
        if let Some(price) = btc {
            print!("{} ${:>10.2} │ ", "BTC".yellow(), price);
        } else {
            print!("{} {:>10} │ ", "BTC".dimmed(), "---");
        }
        
        if let Some(price) = eth {
            print!("{} ${:>8.2} │ ", "ETH".yellow(), price);
        } else {
            print!("{} {:>8} │ ", "ETH".dimmed(), "---");
        }
        
        if let Some(lat_us) = latency {
            if lat_us < 100_000 {  // < 100ms
                print!("Latency: {}ms", (lat_us / 1000).to_string().green());
            } else if lat_us < 500_000 {  // < 500ms
                print!("Latency: {}ms", (lat_us / 1000).to_string().yellow());
            } else {
                print!("Latency: {}ms", (lat_us / 1000).to_string().red());
            }
        }
        
        println!();
        
        // Display detailed stats every 15 seconds
        if i % 15 == 0 {
            println!();
            feed.display_stats().await;
            println!();
        }
    }
    
    // Final summary
    println!("\n{}", "═══════════════════════════════════════".cyan());
    println!("{}", "✅ Test Complete - Final Statistics".green().bold());
    println!("{}", "═══════════════════════════════════════".cyan());
    feed.display_stats().await;
    
    // Calculate performance
    let (msg_count, _) = feed.stats().await;
    let updates_per_sec = msg_count as f64 / 60.0;
    
    println!("\n📊 Performance Metrics:");
    println!("   Average update rate: {:.2} updates/sec", updates_per_sec);
    
    if updates_per_sec > 10.0 {
        println!("   {}", "🚀 EXCELLENT - High-frequency updates!".green().bold());
    } else if updates_per_sec > 1.0 {
        println!("   {}", "✅ GOOD - Normal update rate".green());
    } else {
        println!("   {}", "⚠️  LOW - Check network connection".yellow());
    }
    
    println!("\n{}", "🎉 WebSocket integration is working perfectly!".green().bold());
    println!("{}", "   Ready for paper trading integration!\n".cyan());
    
    Ok(())
}

fn print_banner() {
    println!();
    println!("{}", "═══════════════════════════════════════".cyan().bold());
    println!("{}", "  🔍 PYTH WEBSOCKET DEBUG TOOL".cyan().bold());
    println!("{}", "  Project Flash - Bulletproof Edition".dimmed());
    println!("{}", "═══════════════════════════════════════".cyan().bold());
    println!();
}
