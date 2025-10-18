//! ═══════════════════════════════════════════════════════════════════════════
//! PYTH WEBSOCKET DEBUG TOOL (Hermes V2, October 2025)
//! - Supports multi-feed (SOL/USD, BTC/USD, ETH/USD, etc.)
//! - Prints live stats and diagnoses all common network/feed issues
//! - Bulletproof for Giga tests, battle-ready for development
//! ═══════════════════════════════════════════════════════════════════════════

use solana_grid_bot::trading::{PythWebSocketFeed, load_feed_ids}; // adjust import path!
use tokio::time::{sleep, Duration};
use log::LevelFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    // Enable debug logging
    env_logger::Builder::new()
        .filter_level(LevelFilter::Debug)
        .init();

    println!("\n╔════════════════════════════════════════════════════════╗");
    println!("║        🔍 Pyth Hermes WebSocket Debug Tool           ║");
    println!("╚════════════════════════════════════════════════════════╝\n");

    // Load all feed IDs (modifiable via config, easy scaling)
    let feeds = load_feed_ids();
    println!("🔧 Creating WebSocket feed for {} assets...", feeds.len());

    let feed = PythWebSocketFeed::new(feeds.clone());
    println!("📡 Connecting to Pyth Hermes V2...");
    feed.start().await;

    println!("   (Watch for debug messages below)\n");

    // Wait up to 30 seconds for first message
    for i in 1..=30 {
        sleep(Duration::from_secs(1)).await;
        let (msg_count, connected) = feed.stats().await;
        if msg_count > 0 && connected {
            println!("✅ Message received after {} seconds!", i);
            break;
        }
        if i % 5 == 0 {
            println!("   Still waiting... ({}/30 seconds)", i);
        }
    }

    println!("\n📊 Live statistics:");
    feed.display_stats().await;

    // Print prices for all active feeds
    println!("\n💹 Live prices:");
    for feed_id in &feeds {
        let symbol = feed_id; // Optionally use feed_id_to_symbol
        match feed.get_price(feed_id).await {
            Some(price) => println!("   {symbol}: ${:.4}", price),
            None => println!("   {symbol}: Not available yet"),
        }
    }

    // Final connection check
    let (final_count, connected) = feed.stats().await;
    println!("\n🔩 Feed status: {} | Messages: {}", 
        if connected { "🟢 Connected" } else { "🔴 Disconnected" }, final_count);
    println!("💾 Diagnostic notes:");
    if final_count == 0 {
        println!("   - No price updates received (possible reasons):");
        println!("     1. Network connectivity");
        println!("     2. Hermes API downtime/change");
        println!("     3. Bad or unsupported feed IDs");
        println!("   - Try updating to latest feed IDs or test with mainnet/devnet connectivity.");
    } else {
        println!("   - Price streaming healthy. System ready for grid, trading, dashboard!");
    }

    Ok(())
}
