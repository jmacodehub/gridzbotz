//! Trading module - Price feeds and execution

pub use crate::config::Config;

pub mod price_feed;
pub mod pyth_price_feed;
pub mod pyth_http;
pub mod paper_trader;

#[cfg(feature = "websockets")]
pub mod pyth_websocket;

pub use paper_trader::{
    PaperTradingEngine, 
    VirtualWallet, 
    Order, 
    OrderSide, 
    OrderStatus,
    OrderType,
    Trade,
    PerformanceStats,
};

pub use price_feed::{PriceFeed, PriceFeedMetrics, FeedMode};
pub use pyth_http::{PythHttpFeed, PriceUpdate as HttpPriceUpdate, feed_ids as http_feed_ids};
pub use pyth_price_feed::PythPriceFeed;

#[cfg(feature = "websockets")]
pub use pyth_websocket::{PythWebSocketFeed, PriceUpdate as WsPriceUpdate};

#[cfg(feature = "websockets")]
pub type LivePriceUpdate = WsPriceUpdate;
#[cfg(not(feature = "websockets"))]
pub type LivePriceUpdate = HttpPriceUpdate;

#[cfg(feature = "websockets")]
pub use pyth_http::feed_ids as live_feed_ids;
#[cfg(not(feature = "websockets"))]
pub use http_feed_ids as live_feed_ids;

pub async fn get_live_price(feed_id: &str) -> Option<f64> {
    let http = PythHttpFeed::new(vec![feed_id.to_string()]);
    if let Ok(_) = http.start().await {
        http.get_price(feed_id).await
    } else {
        None
    }
}
