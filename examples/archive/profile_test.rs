//! Performance Profiler - Benchmark all components

use solana_grid_bot::trading::{PythHttpFeed, feed_ids, PaperTradingEngine, OrderSide};
use std::time::Instant;
use colored::*;

#[tokio::main]
async fn main() {
    println!("\n{}", "=== PERFORMANCE PROFILER - PROJECT FLASH V2.5 ===".bright_cyan().bold());
    println!();
    
    let total_start = Instant::now();
    
    // 1. Price Feed Performance
    println!("{}", "Price Feed Benchmark:".bright_yellow());
    let feed = PythHttpFeed::new(vec![feed_ids::SOL_USD.to_string()]);
    feed.start().await.unwrap();
    
    let mut fetch_times = vec![];
    for i in 0..100 {
        let start = Instant::now();
        let _ = feed.get_price(feed_ids::SOL_USD).await;
        fetch_times.push(start.elapsed().as_micros());
        
        if i % 25 == 24 {
            let batch_avg = fetch_times[i-24..=i].iter().sum::<u128>() as f64 / 25.0 / 1000.0;
            println!("  Batch {}: {:.2}ms avg", i/25 + 1, batch_avg);
        }
    }
    
    let avg = fetch_times.iter().sum::<u128>() as f64 / 100.0 / 1000.0;
    println!("  {} {:.2}ms\n", "Average fetch:".bright_green(), avg);
    
    // 2. Order Placement Performance
    println!("{}", "Order Placement Benchmark:".bright_yellow());
    let engine = PaperTradingEngine::new(100000.0, 100.0);
    
    let start = Instant::now();
    for i in 0..1000 {
        let _ = engine.place_limit_order(
            if i % 2 == 0 { OrderSide::Buy } else { OrderSide::Sell },
            150.0 + i as f64 * 0.1,
            1.0
        ).await;
    }
    let elapsed = start.elapsed();
    
    println!("  1000 orders: {:.2}ms ({:.0} orders/sec)", 
        elapsed.as_millis(),
        1000.0 / elapsed.as_secs_f64()
    );
    println!("  {} {:.2}μs\n", "Average per order:".bright_green(), 
        elapsed.as_micros() as f64 / 1000.0);
    
    // 3. Price Update Processing
    println!("{}", "Price Update Benchmark:".bright_yellow());
    let start = Instant::now();
    for i in 0..1000 {
        let _ = engine.process_price_update(150.0 + (i as f64 * 0.01)).await;
    }
    let elapsed = start.elapsed();
    
    println!("  1000 updates: {:.2}ms", elapsed.as_millis());
    println!("  {} {:.2}μs\n", "Average per update:".bright_green(), 
        elapsed.as_micros() as f64 / 1000.0);
    
    feed.stop().await;
    
    println!("{}", "=== BENCHMARK COMPLETE ===".bright_green().bold());
    println!("Total time: {:.2}s\n", total_start.elapsed().as_secs_f64());
}
