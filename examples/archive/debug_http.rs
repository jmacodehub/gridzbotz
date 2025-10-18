//! Debug tool for testing Pyth HTTP price feeds
//! Shows live prices updating in real-time

use solana_grid_bot::trading::pyth_http::{PythHttpFeed, feed_ids};
use tokio::time::{sleep, Duration};

#[tokio::main]
async fn main() {
    // Initialize logging
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();
    
    println!("\n═══════════════════════════════════════");
    println!("  🔍 PYTH HTTP DEBUG TOOL");
    println!("  Project Flash - Bulletproof Edition");
    println!("═══════════════════════════════════════\n");
    
    // Create feed with all three major pairs
    let feed = PythHttpFeed::new(vec![
        feed_ids::SOL_USD.to_string(),
        feed_ids::BTC_USD.to_string(),
        feed_ids::ETH_USD.to_string(),
    ]);
    
    // Start polling
    feed.start().await.expect("Failed to start feed");
    
    println!("✅ Connected successfully!\n");
    println!("⏳ Monitoring prices for 60 seconds...");
    println!("Press Ctrl+C to stop\n");
    
    // Monitor for 60 seconds
    for i in 1..=60 {
        sleep(Duration::from_secs(1)).await;
        
        // Get current prices using the CORRECT feed IDs
        let sol_price = feed.get_price(feed_ids::SOL_USD).await;
        let btc_price = feed.get_price(feed_ids::BTC_USD).await;
        let eth_price = feed.get_price(feed_ids::ETH_USD).await;
        
        // Display nicely formatted
        print!("\r⏱️  {:3}s │ ", i);
        
        if let Some(p) = sol_price {
            print!("SOL ${:8.4} │ ", p);
        } else {
            print!("SOL      --- │ ");
        }
        
        if let Some(p) = btc_price {
            print!("BTC ${:10.2} │ ", p);
        } else {
            print!("BTC        --- │ ");
        }
        
        if let Some(p) = eth_price {
            print!("ETH ${:8.4}", p);
        } else {
            print!("ETH      ---");
        }
        
        // Show stats every 15 seconds
        if i % 15 == 0 {
            println!("\n");
            feed.display_stats().await;
            println!();
        }
    }
    
    println!("\n\n═══════════════════════════════════════");
    println!("✅ Test Complete");
    println!("═══════════════════════════════════════\n");
    
    feed.display_stats().await;
    
    // Stop feed
    feed.stop().await;
}
