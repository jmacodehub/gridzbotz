//! ═════════════════════════════════════════════════════════════════════════
//! Trading Module V5.1 - Jupiter-Powered Real Trading Engine (Consolidated)
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
//! February 2026 - V5.1 JUPITER CONSOLIDATED! 🚀
//! ═════════════════════════════════════════════════════════════════════════

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
    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<String>>;
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
    fn test_module_exports() {
        use super::prelude::*;
        let _: Option<RealTradingConfig> = None;
        let _: Option<EnhancedMetrics>   = None;
        let _: Option<AdaptiveOptimizer> = None;
        let _: Option<JupiterClient>     = None;
    }
}
