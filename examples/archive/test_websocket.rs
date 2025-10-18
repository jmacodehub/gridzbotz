//! Test WebSocket price feed

use solana_grid_bot::trading::PythWebSocketFeed;
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::init();
    
    println!("\n╔═══════════════════════════════════════════════════╗");
    println!("║  🌐 Pyth WebSocket Price Feed - Live Test       ║");
    println!("╚═══════════════════════════════════════════════════╝\n");
    
    // Create and start WebSocket
    println!("🔧 Creating WebSocket feed...");
    let feed = PythWebSocketFeed::new();
    
    println!("📡 Connecting to Pyth Network...");
    feed.start().await?;
    
    println!("✅ WebSocket connected!\n");
    println!("📊 Watching real-time price updates for 15 seconds...\n");
    
    // Watch updates for 15 seconds
    let mut last_price = 0.0;
    for i in 1..=15 {
        sleep(Duration::from_secs(1)).await;
        
        if let Some(price) = feed.get_price().await {
            let change = if last_price > 0.0 {
                ((price - last_price) / last_price) * 100.0
            } else {
                0.0
            };
            
            let emoji = if change > 0.0 {
                "📈"
            } else if change < 0.0 {
                "📉"
            } else {
                "➡️"
            };
            
            println!("  {} Second {:2}: SOL/USD ${:.4} ({:+.3}%)", 
                     emoji, i, price, change);
            
            last_price = price;
        } else {
            println!("  ⏳ Second {:2}: Waiting for data...", i);
        }
    }
    
    println!("\n");
    feed.display_stats().await;
    
    println!("\n✅ WebSocket test completed successfully!");
    
    Ok(())
}
