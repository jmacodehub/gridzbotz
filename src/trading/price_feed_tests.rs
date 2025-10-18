use solana_grid_bot::trading::PriceFeed;

#[tokio::test]
async fn test_record_and_volatility() {
    let feed = PriceFeed::new("wss://invalid.url", 5);
    // Directly record some prices
    for &p in &[100.0, 102.0, 101.0, 103.0, 99.0] {
        feed.record_price(p).await;
    }
    let vol = feed.volatility().await;
    assert!(vol > 0.0);
}