// examples/one_hour_pyth_test.rs

use solana_grid_bot::trading::PythWebSocketFeed; // Or import path
use std::fs::OpenOptions;
use std::io::Write;
use tokio::time::{sleep, Duration, Instant};
use log::LevelFilter;

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    env_logger::Builder::new().filter_level(LevelFilter::Info).init();
    let feed = PythWebSocketFeed::default();
    feed.start().await;
    let start = Instant::now();

    // Open log file
    let mut file = OpenOptions::new()
        .append(true).create(true).open("pyth_ws_1h.log")?;

    while start.elapsed() < Duration::from_secs(3600) {
        let stats = feed.stats().await;
        let all_prices = feed.get_all_prices().await;
        let now = chrono::Utc::now();
        let msg = format!(
            "[{}] {:?} | {:?}\n", now, stats, all_prices
        );
        print!("{}", msg);
        file.write_all(msg.as_bytes())?;
        file.flush()?;
        sleep(Duration::from_secs(60)).await;
    }

    println!("Test finished after 1 hour. Data saved to pyth_ws_1h.log");
    Ok(())
}
