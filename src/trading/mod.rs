//! ═════════════════════════════════════════════════════════════════════
//! Trading Module V5.2 - Jupiter-Powered Real Trading Engine (Consolidated)
//!
//! Architecture:
//! - Unified Trading Interface: Generic trait for paper and live trading
//! - Paper Trading: Risk-free backtesting and simulation
//! - Grid State Machine: Order lifecycle tracking with buy/sell pairing
//! - Real Trading: ✅ ENABLED - Live execution with Jupiter swaps!
//! - Jupiter Client: V5.1 — Single unified client, ALTs preserved, dynamic fees
//! - Price Feeds: Multiple sources with redundancy and consensus
//! - Transaction Executor: Solana transaction building and signing
//! - Enhanced Metrics: Trade-level analytics and performance tracking
//! - Adaptive Optimizer: Self-learning grid spacing and position sizing
//!
//! V5.1 CHANGES (Feb 2026):
//! ✅ jupiter_swap.rs removed — consolidated into jupiter_client.rs
//! ✅ JupiterClient is now the single canonical implementation
//! ✅ VersionedTransaction preserved end-to-end (ALTs no longer dropped)
//! ✅ executor.execute_versioned() wired for Jupiter swaps
//! ✅ keystore.sign_versioned_transaction() added
//!
//! V5.2 CHANGES (Stage 3 — Feb 2026):
//! ✅ FillEvent added — data carrier for confirmed order fills
//!    Fan-out to strategies via StrategyManager::notify_fill()
//!
//! V5.2.1 CHANGES (Feb 2026 — per-level analytics):
//! ✅ FillEvent gains level_id: Option<u64>            — grid level ID
//! ✅ FillEvent gains distance_from_mid_pct: Option<f64> — % from mid at fill
//! ✅ Builder methods: .with_level() / .with_distance_from_mid()
//! ✅ All existing call sites unaffected (new fields default to None)
//!
//! February 2026 - V5.2.1 PER-LEVEL ANALYTICS! 🚀
//! ═════════════════════════════════════════════════════════════════════

pub use crate::config::Config;

// Re-export async_trait for trait implementations
pub use async_trait::async_trait;

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
pub mod jupiter_client;      // 🪐 V5.1: Unified Jupiter client (replaces jupiter_swap)
pub mod real_trader;         // 🔥 ENABLED - Phase 5 Complete!
pub mod enhanced_metrics;    // 📊 V4.1: Enhanced analytics tracking
pub mod adaptive_optimizer;  // 🧠 V4.2: Self-learning optimizer

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
// Enhanced Metrics Exports (V4.1) 📊
// ═══════════════════════════════════════════════════════════════════════════

pub use enhanced_metrics::EnhancedMetrics;

// ═══════════════════════════════════════════════════════════════════════════
// Adaptive Optimizer Exports (V4.2) 🧠
// ═══════════════════════════════════════════════════════════════════════════

pub use adaptive_optimizer::{
    AdaptiveOptimizer,
    OptimizationResult,
};

// ═══════════════════════════════════════════════════════════════════════════
// Jupiter Client Exports (V5.1) 🪐 — Unified, ALTs preserved
// ═══════════════════════════════════════════════════════════════════════════

pub use jupiter_client::{
    JupiterClient,
    JupiterConfig,
    JupiterQuoteRequest,
    JupiterQuoteResponse,
    JupiterSwapRequest,
    JupiterSwapResponse,
    PriorityFee,
    PriorityLevelWithMaxLamports,
    SOL_MINT,
    WSOL_MINT,      // Backwards-compat alias for SOL_MINT
    USDC_MINT,
};

// ═══════════════════════════════════════════════════════════════════════════
// Real Trading Exports (🔥 ENABLED - Phase 5!)
// ═══════════════════════════════════════════════════════════════════════════

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
    ExecutionStats,
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
// UNIFIED TRADING ENGINE TRAIT (V4.1) 🚀
// ═══════════════════════════════════════════════════════════════════════════

/// Result type for trading operations
pub type TradingResult<T> = anyhow::Result<T>;

/// Order placement result with order ID and optional metadata
#[derive(Debug, Clone)]
pub struct OrderPlacementResult {
    pub order_id: String,
    pub signature: Option<String>,
    pub estimated_price: Option<f64>,
    pub estimated_fees: Option<f64>,
}

impl OrderPlacementResult {
    pub fn simple(order_id: String) -> Self {
        Self { order_id, signature: None, estimated_price: None, estimated_fees: None }
    }

    pub fn detailed(order_id: String, signature: String, estimated_price: f64, estimated_fees: f64) -> Self {
        Self {
            order_id,
            signature: Some(signature),
            estimated_price: Some(estimated_price),
            estimated_fees: Some(estimated_fees),
        }
    }
}

/// Batch order operation for efficient multi-order placement
#[derive(Debug, Clone)]
pub struct BatchOrderRequest {
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub grid_level_id: Option<u64>,
}

// ═══════════════════════════════════════════════════════════════════════════
// FILL EVENT (V5.2 / V5.2.1) 📨
//
// Emitted whenever an order is confirmed filled.
// Fan-out to all strategies via StrategyManager::notify_fill().
//
// V5.2.1: Two optional analytics fields added for per-level tracking.
// All existing FillEvent::new() call sites are unaffected — new fields
// default to None and can be attached fluently via builder methods:
//
//   FillEvent::new(...).with_level(level.id).with_distance_from_mid(-1.2)
//
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct FillEvent {
    /// Unique order identifier
    pub order_id:   String,
    /// Buy or Sell
    pub side:       OrderSide,
    /// Actual fill price
    pub fill_price: f64,
    /// Amount filled (in base token, e.g. SOL)
    pub fill_size:  f64,
    /// Transaction fee paid in USDC
    pub fee_usdc:   f64,
    /// Realised P&L for this fill (None if unknown / first leg)
    pub pnl:        Option<f64>,
    /// Unix timestamp (seconds)
    pub timestamp:  i64,

    // ── V5.2.1: Per-level analytics ───────────────────────────────────────
    /// Grid level that triggered this fill.
    /// Matches `GridLevel.id` (u64) exactly.
    /// `None` for non-grid fills (manual trades, RSI/Momentum signals).
    pub level_id:              Option<u64>,
    /// Percentage distance from mid-price at the moment of fill.
    /// Negative = fill below mid (buy side), positive = above (sell side).
    /// Example: -1.2 means the fill occurred 1.2% below mid-price.
    /// `None` when mid-price was unavailable at fill time.
    pub distance_from_mid_pct: Option<f64>,
}

impl FillEvent {
    /// Construct a FillEvent.
    ///
    /// Pass `timestamp` from the engine clock
    /// (`chrono::Utc::now().timestamp()` or a mock for tests).
    ///
    /// `level_id` and `distance_from_mid_pct` default to `None`.
    /// Attach them with the builder methods below:
    ///   `.with_level(id)` and `.with_distance_from_mid(pct)`
    pub fn new(
        order_id:   impl Into<String>,
        side:       OrderSide,
        fill_price: f64,
        fill_size:  f64,
        fee_usdc:   f64,
        pnl:        Option<f64>,
        timestamp:  i64,
    ) -> Self {
        Self {
            order_id: order_id.into(),
            side,
            fill_price,
            fill_size,
            fee_usdc,
            pnl,
            timestamp,
            level_id:              None,
            distance_from_mid_pct: None,
        }
    }

    // ── V5.2.1: Builder methods ────────────────────────────────────────

    /// Attach the grid level that triggered this fill.
    ///
    /// Use the `GridLevel.id` value directly.
    ///
    /// # Examples
    ///
    /// ```
    /// use solana_grid_bot::trading::{FillEvent, OrderSide};
    ///
    /// let level_id: u64 = 42;
    /// let fill = FillEvent::new(
    ///     "ORDER-123",
    ///     OrderSide::Buy,
    ///     153.50,
    ///     0.1,
    ///     0.0025,
    ///     None,
    ///     1_700_000_000,
    /// ).with_level(level_id);
    ///
    /// assert_eq!(fill.level_id, Some(42));
    /// ```
    #[inline]
    pub fn with_level(mut self, level_id: u64) -> Self {
        self.level_id = Some(level_id);
        self
    }

    /// Attach the percentage distance from mid-price at fill time.
    ///
    /// - Negative value → fill occurred below mid (buy side)
    /// - Positive value → fill occurred above mid (sell side)
    ///
    /// # Examples
    ///
    /// ```
    /// use solana_grid_bot::trading::{FillEvent, OrderSide};
    ///
    /// let mid = 155.00_f64;
    /// let fill_price = 153.14_f64;
    /// let pct = (fill_price - mid) / mid * 100.0;
    ///
    /// let fill = FillEvent::new(
    ///     "ORDER-456",
    ///     OrderSide::Buy,
    ///     fill_price,
    ///     0.1,
    ///     0.0025,
    ///     None,
    ///     1_700_000_100,
    /// ).with_distance_from_mid(pct);
    ///
    /// assert!(fill.distance_from_mid_pct.unwrap() < 0.0, "Buy below mid should be negative");
    /// ```
    #[inline]
    pub fn with_distance_from_mid(mut self, pct: f64) -> Self {
        self.distance_from_mid_pct = Some(pct);
        self
    }
}

/// Engine health status for monitoring
#[derive(Debug, Clone)]
pub struct EngineHealthStatus {
    pub is_healthy: bool,
    pub trading_allowed: bool,
    pub circuit_breaker_active: bool,
    pub last_successful_trade: Option<i64>,
    pub error_rate: f64,
    pub message: String,
}

/// Unified trading engine interface for paper and live trading
#[async_trait]
pub trait TradingEngine: Send + Sync {
    async fn place_limit_order_with_level(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
        grid_level_id: Option<u64>,
    ) -> TradingResult<String>;

    async fn cancel_order(&self, order_id: &str) -> TradingResult<()>;
    async fn cancel_all_orders(&self) -> TradingResult<usize>;

    /// Process price update and return any newly confirmed fills.
    ///
    /// V5.2: Returns Vec<FillEvent> (not Vec<String>) to support fill fan-out.
    /// Each FillEvent is broadcast to all strategies via StrategyManager::notify_fill().
    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>>;

    async fn open_order_count(&self) -> usize;
    async fn is_trading_allowed(&self) -> bool;

    async fn health_check(&self) -> EngineHealthStatus {
        EngineHealthStatus {
            is_healthy: true,
            trading_allowed: self.is_trading_allowed().await,
            circuit_breaker_active: !self.is_trading_allowed().await,
            last_successful_trade: None,
            error_rate: 0.0,
            message: "Engine operational".to_string(),
        }
    }

    async fn place_batch_orders(
        &self,
        orders: Vec<BatchOrderRequest>,
    ) -> TradingResult<Vec<OrderPlacementResult>> {
        let mut results = Vec::with_capacity(orders.len());
        for order in orders {
            match self.place_limit_order_with_level(order.side, order.price, order.size, order.grid_level_id).await {
                Ok(order_id) => results.push(OrderPlacementResult::simple(order_id)),
                Err(e) => log::warn!("Batch order failed: {}", e),
            }
        }
        Ok(results)
    }

    async fn get_order_details(&self, _order_id: &str) -> Option<Order> { None }
    async fn estimate_execution_price(&self, _side: OrderSide, _size: f64) -> Option<f64> { None }

    async fn emergency_shutdown(&self, _reason: &str) -> TradingResult<()> {
        log::error!("🚨 EMERGENCY SHUTDOWN - {}", _reason);
        self.cancel_all_orders().await?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// Helper Functions
// ═══════════════════════════════════════════════════════════════════════════

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

pub mod prelude {
    pub use super::{
        // Engines
        PaperTradingEngine,
        RealTradingEngine,
        TradingEngine,

        // Orders & Types
        OrderSide,
        OrderStatus,
        OrderType,
        Order,

        // Fill Event (V5.2 / V5.2.1 per-level analytics)
        FillEvent,

        // Grid State Machine
        GridStateTracker,
        GridLevel,
        GridLevelStatus,

        // Price Feeds
        PriceFeed,
        FeedMode,

        // Jupiter V5.1 — unified
        JupiterClient,
        JupiterConfig,
        JupiterQuoteResponse,
        SOL_MINT,
        WSOL_MINT,
        USDC_MINT,

        // Results
        TradingResult,
        OrderPlacementResult,
        EngineHealthStatus,

        // Real Trading
        RealTradingConfig,
        RealPerformanceStats,

        // Enhanced Metrics
        EnhancedMetrics,

        // Adaptive Optimizer
        AdaptiveOptimizer,
        OptimizationResult,
    };
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_order_placement_result_simple() {
        let result = OrderPlacementResult::simple("ORDER-123".to_string());
        assert_eq!(result.order_id, "ORDER-123");
        assert!(result.signature.is_none());
    }

    #[test]
    fn test_order_placement_result_detailed() {
        let result = OrderPlacementResult::detailed(
            "ORDER-456".to_string(), "SIG-789".to_string(), 200.50, 0.10,
        );
        assert_eq!(result.signature.unwrap(), "SIG-789");
        assert_eq!(result.estimated_price.unwrap(), 200.50);
    }

    #[test]
    fn test_fill_event_construction() {
        let fill = FillEvent::new(
            "ORDER-BUY-001",
            OrderSide::Buy,
            142.50,
            0.1,
            0.0025,
            Some(0.05),
            1_700_000_000,
        );
        assert_eq!(fill.order_id, "ORDER-BUY-001");
        assert_eq!(fill.fill_price, 142.50);
        assert_eq!(fill.fill_size, 0.1);
        assert_eq!(fill.pnl, Some(0.05));
        assert_eq!(fill.timestamp, 1_700_000_000);
        // V5.2.1: new fields must be None by default — existing callers unaffected
        assert!(fill.level_id.is_none(), "level_id must default to None");
        assert!(fill.distance_from_mid_pct.is_none(), "distance_from_mid_pct must default to None");
    }

    #[test]
    fn test_fill_event_no_pnl() {
        // First leg of a grid pair has no P&L yet
        let fill = FillEvent::new(
            "ORDER-SELL-002",
            OrderSide::Sell,
            143.00,
            0.1,
            0.0025,
            None,
            1_700_000_001,
        );
        assert!(fill.pnl.is_none());
        assert!(fill.level_id.is_none());
        assert!(fill.distance_from_mid_pct.is_none());
    }

    // ── V5.2.1: Per-level analytics tests ───────────────────────────────────────

    #[test]
    fn test_fill_event_with_level_id() {
        let fill = FillEvent::new(
            "ORDER-BUY-042",
            OrderSide::Buy,
            153.50,
            0.1,
            0.0025,
            None,
            1_700_000_100,
        )
        .with_level(102);

        assert_eq!(fill.level_id, Some(102));
        assert!(fill.distance_from_mid_pct.is_none());
        assert_eq!(fill.order_id, "ORDER-BUY-042");
        assert_eq!(fill.fill_price, 153.50);
    }

    #[test]
    fn test_fill_event_builder_chain() {
        // Simulate a real grid fill: level 3, price 1.2% below mid of 155.00
        let mid_price = 155.00_f64;
        let fill_price = 153.14_f64;
        let level_id: u64 = 3;
        let distance_pct = (fill_price - mid_price) / mid_price * 100.0;

        let fill = FillEvent::new(
            "ORDER-BUY-003",
            OrderSide::Buy,
            fill_price,
            0.2,
            0.003,
            Some(1.85),
            1_700_000_200,
        )
        .with_level(level_id)
        .with_distance_from_mid(distance_pct);

        // All core fields intact
        assert_eq!(fill.order_id, "ORDER-BUY-003");
        assert_eq!(fill.side, OrderSide::Buy);
        assert_eq!(fill.fill_price, fill_price);
        assert_eq!(fill.pnl, Some(1.85));

        // Analytics fields correctly attached
        assert_eq!(fill.level_id, Some(3));
        let dist = fill.distance_from_mid_pct.unwrap();
        assert!(dist < 0.0, "Buy below mid must be negative: got {:.4}", dist);
        assert!((dist - (-1.2)).abs() < 0.1, "Expected ~-1.2%, got {:.4}%", dist);
    }

    #[test]
    fn test_fill_event_sell_side_distance() {
        // Sell fill above mid → positive distance
        let mid_price = 155.00_f64;
        let fill_price = 156.55_f64;
        let distance_pct = (fill_price - mid_price) / mid_price * 100.0;

        let fill = FillEvent::new(
            "ORDER-SELL-201",
            OrderSide::Sell,
            fill_price,
            0.1,
            0.002,
            Some(3.10),
            1_700_000_300,
        )
        .with_level(201)
        .with_distance_from_mid(distance_pct);

        assert_eq!(fill.level_id, Some(201));
        let dist = fill.distance_from_mid_pct.unwrap();
        assert!(dist > 0.0, "Sell above mid must be positive: got {:.4}", dist);
        assert!((dist - 1.0).abs() < 0.1, "Expected ~+1.0%, got {:.4}%", dist);
    }

    #[test]
    fn test_module_exports() {
        use super::prelude::*;
        let _: Option<RealTradingConfig> = None;
        let _: Option<EnhancedMetrics>   = None;
        let _: Option<AdaptiveOptimizer> = None;
        let _: Option<JupiterClient>     = None;
        let _: Option<FillEvent>         = None;
    }
}
