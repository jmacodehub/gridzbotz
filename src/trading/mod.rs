//! ═══════════════════════════════════════════════════════════════════════════
//! Trading Module V4.0 - Price Feeds, Execution Engines, Grid State Management
//!
//! Architecture:
//! - Paper Trading: Risk-free backtesting and simulation
//! - Grid State Machine: Order lifecycle tracking with buy/sell pairing
//! - Real Trading: Live execution with circuit breakers (when security module ready)
//! - Price Feeds: Multiple sources with redundancy and consensus
//! - Transaction Executor: Solana transaction building and signing
//!
//! February 7, 2026 - V4.0 with Grid Level State Machine
//! ═══════════════════════════════════════════════════════════════════════════

pub use crate::config::Config;

// ═══════════════════════════════════════════════════════════════════════════
// Core Trading Modules
// ═══════════════════════════════════════════════════════════════════════════

pub mod price_feed;
pub mod pyth_price_feed;
pub mod pyth_http;
pub mod paper_trader;
pub mod grid_level;          // V4.0: Grid state machine
pub mod executor;            // Transaction executor
pub mod trade;               // Trade data structures
pub mod feed_consensus;      // Feed consensus logic
pub mod redundant_feed;      // Redundant price feeds

// Real trading engine (conditionally compiled when security module exists)
#[cfg(all(feature = "live-trading", feature = "security"))]
pub mod real_trader;

// WebSocket feeds (optional feature)
#[cfg(feature = "websockets")]
pub mod pyth_websocket;
#[cfg(feature = "websockets")]
pub mod binance_ws;
#[cfg(feature = "websockets")]
pub mod pyth_lazer;

// ═══════════════════════════════════════════════════════════════════════════
// Paper Trading Exports
// ═══════════════════════════════════════════════════════════════════════════

pub use paper_trader::{
    PaperTradingEngine,
    VirtualWallet,
    Order,
    OrderSide,
    OrderStatus,
    OrderType,
    Trade as PaperTrade,
    PerformanceStats as PaperPerformanceStats,
};

// ═══════════════════════════════════════════════════════════════════════════
// Grid Level State Machine Exports (V4.0)
// ═══════════════════════════════════════════════════════════════════════════

pub use grid_level::{
    GridLevel,
    GridLevelStatus,
    GridStateTracker,
};

// ═══════════════════════════════════════════════════════════════════════════
// Real Trading Exports (Conditional)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(all(feature = "live-trading", feature = "security"))]
pub use real_trader::{
    RealTradingEngine,
    RealTradingConfig,
    PerformanceStats as RealPerformanceStats,
};

// ═══════════════════════════════════════════════════════════════════════════
// Transaction Executor Exports
// ═══════════════════════════════════════════════════════════════════════════

pub use executor::{
    TransactionExecutor,
    ExecutorConfig,
    ExecutionStats,  // 🔥 FIXED: Was ExecutorStats, now ExecutionStats
};

pub use trade::Trade;

// ═══════════════════════════════════════════════════════════════════════════
// Price Feed Exports
// ═══════════════════════════════════════════════════════════════════════════

pub use price_feed::{PriceFeed, PriceFeedMetrics, FeedMode};
pub use pyth_http::{PythHttpFeed, PriceUpdate as HttpPriceUpdate, feed_ids as http_feed_ids};
pub use pyth_price_feed::PythPriceFeed;

#[cfg(feature = "websockets")]
pub use pyth_websocket::{PythWebSocketFeed, PriceUpdate as WsPriceUpdate};

// ═══════════════════════════════════════════════════════════════════════════
// Conditional Type Aliases (WebSocket vs HTTP)
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(feature = "websockets")]
pub type LivePriceUpdate = WsPriceUpdate;
#[cfg(not(feature = "websockets"))]
pub type LivePriceUpdate = HttpPriceUpdate;

#[cfg(feature = "websockets")]
pub use pyth_http::feed_ids as live_feed_ids;
#[cfg(not(feature = "websockets"))]
pub use http_feed_ids as live_feed_ids;

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

/// Quick price fetch utility (HTTP fallback)
pub async fn get_live_price(feed_id: &str) -> Option<f64> {
    let http = PythHttpFeed::new(vec![feed_id.to_string()]);
    if http.start().await.is_ok() {
        http.get_price(feed_id).await
    } else {
        None
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Re-exports for Convenience
// ═══════════════════════════════════════════════════════════════════════════

/// Common types for external use
pub mod prelude {
    pub use super::{
        PaperTradingEngine,
        OrderSide,
        OrderStatus,
        GridStateTracker,
        GridLevel,
        GridLevelStatus,
        PriceFeed,
        FeedMode,
    };
}

// ═══════════════════════════════════════════════════════════════════════════
// Feature Flags Documentation
// ═══════════════════════════════════════════════════════════════════════════

// Real trading engine is conditionally compiled when live-trading + security features enabled.
// This gracefully allows paper trading without the security module.

#[cfg(all(doc, not(feature = "live-trading")))]
/// **DOCUMENTATION NOTE:**
///
/// The `RealTradingEngine` requires the `live-trading` and `security` features.
///
/// Enable with: `cargo build --features live-trading,security`
pub struct RealTradingEngineDocumentation;


#[cfg(all(doc, not(feature = "live-trading")))]
/// **DOCUMENTATION NOTE:**
///
/// The `RealTradingEngine` is not compiled by default for safety.
///
/// ## Enabling Live Trading
///
/// ```toml
/// # Cargo.toml
/// [features]
/// live-trading = []
/// security = []
/// ```
///
/// ```bash
/// cargo build --features live-trading,security
/// ```
///
/// ## Safety Requirements
///
/// Before enabling live trading:
/// 1. ✅ Implement secure keystore in `src/security/keystore.rs`
/// 2. ✅ Configure circuit breaker limits
/// 3. ✅ Test extensively in paper mode
/// 4. ✅ Review risk management settings
/// 5. ✅ Verify RPC endpoints and API keys
pub struct RealTradingEngineDocumentation;
