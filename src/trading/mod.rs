//! ═══════════════════════════════════════════════════════════════════════════
//! Trading Module V5.0 - Unified Trading Engine with MEV Protection
//!
//! Architecture:
//! - Unified Trading Interface: Generic trait for paper and live trading
//! - Paper Trading: Risk-free backtesting and simulation
//! - Grid State Machine: Order lifecycle tracking with buy/sell pairing
//! - Real Trading: ✅ ENABLED - Live execution with Jupiter swaps!
//! - Jupiter Integration: Cross-DEX swaps via Jupiter aggregator (🪐)
//! - Price Feeds: Multiple sources with redundancy and consensus
//! - Transaction Executor: Solana transaction building and signing
//! - Enhanced Metrics: Trade-level analytics and performance tracking
//! - Adaptive Optimizer: Self-learning grid spacing and position sizing
//! - MEV Protection: 🛡️ NEW! Priority fees, slippage guard, Jito bundles
//!
//! V5.0 ENHANCEMENTS:
//! ✅ TradingEngine trait - unified interface for all trading modes
//! ✅ Grid level ID tracking in orders
//! ✅ Circuit breaker integration
//! ✅ Extensible for future order types (stop-loss, take-profit, etc.)
//! ✅ Batch order operations for efficiency
//! ✅ Jupiter Swap integration for live trading (🆕)
//! ✅ RealTradingEngine ENABLED with full security (🔥 Phase 5)
//! ✅ Enhanced Metrics for deep analytics (📊 V4.1)
//! ✅ Adaptive Optimizer for self-learning (🧠 V4.2)
//! 🔥 MEV Protection - Priority fees, slippage guard, Jito bundles (🛡️ V5.0)
//!
//! February 11, 2026 - V5.0 MEV PROTECTION INTEGRATED!
//! ═══════════════════════════════════════════════════════════════════════════

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
pub mod jupiter_swap;        // 🪐 Jupiter DEX aggregator (V4.1)
pub mod real_trader;         // 🔥 ENABLED - Phase 5 Complete!
pub mod enhanced_metrics;    // 📊 V4.1: Enhanced analytics tracking
pub mod adaptive_optimizer;  // 🧠 V4.2: Self-learning optimizer
pub mod mev_protection;      // 🛡️ V5.0: MEV Protection (NEW!)

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
// MEV Protection Exports (V5.0) 🛡️ NEW!
// ═══════════════════════════════════════════════════════════════════════════

pub use mev_protection::{
    // Main manager
    MevProtectionManager,
    MevProtectionConfig,
    
    // Priority fees
    PriorityFeeOptimizer,
    PriorityFeeConfig,
    FeeRecommendation,
    
    // Slippage protection
    SlippageGuardian,
    SlippageConfig,
    SlippageValidation,
    
    // Jito bundles
    JitoClient,
    JitoConfig,
    JitoBundleStatus,
};

// ═══════════════════════════════════════════════════════════════════════════
// Jupiter Swap Exports (V4.1) 🪐
// ═══════════════════════════════════════════════════════════════════════════

pub use jupiter_swap::{
    JupiterSwapClient,
    QuoteResponse,        // ✅ FIXED: Was JupiterQuote
    SwapRequest,          // ✅ FIXED: Was JupiterSwapRequest
    SwapResponse,         // ✅ FIXED: Was JupiterSwapResponse
    WSOL_MINT,
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
    /// Unique order identifier
    pub order_id: String,
    /// Transaction signature (for live trading)
    pub signature: Option<String>,
    /// Estimated execution price
    pub estimated_price: Option<f64>,
    /// Estimated fees
    pub estimated_fees: Option<f64>,
}

impl OrderPlacementResult {
    /// Create simple result with just order ID
    pub fn simple(order_id: String) -> Self {
        Self {
            order_id,
            signature: None,
            estimated_price: None,
            estimated_fees: None,
        }
    }

    /// Create detailed result with all metadata
    pub fn detailed(
        order_id: String,
        signature: String,
        estimated_price: f64,
        estimated_fees: f64,
    ) -> Self {
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
///
/// # Design Philosophy
///
/// This trait provides a unified interface that works across:
/// - Paper trading (simulation mode)
/// - Live trading (real money on Solana DEX via Jupiter)
/// - Backtesting engines
/// - Mock engines for testing
///
/// # Safety Guarantees
///
/// Implementations MUST guarantee:
/// - Thread safety (Send + Sync)
/// - Atomic order placement (no partial states)
/// - Circuit breaker integration
/// - Proper error propagation
///
/// # Future Extensions
///
/// This trait is designed to support:
/// - Stop-loss orders (V4.2)
/// - Take-profit orders (V4.2)
/// - Trailing stops (V4.3)
/// - Advanced order types (iceberg, TWAP, etc.)
/// - Multi-DEX routing (V5.0)
#[async_trait]
pub trait TradingEngine: Send + Sync {
    // ═════════════════════════════════════════════════════════════════════════
    // CORE ORDER OPERATIONS (Required)
    // ═════════════════════════════════════════════════════════════════════════

    /// Place limit order with optional grid level tracking
    ///
    /// # Arguments
    /// * `side` - Buy or Sell
    /// * `price` - Limit price in USD
    /// * `size` - Order size in base token (SOL)
    /// * `grid_level_id` - Optional grid level for state machine tracking
    ///
    /// # Returns
    /// Order ID that can be used for cancellation and tracking
    ///
    /// # Errors
    /// - Insufficient balance
    /// - Circuit breaker tripped
    /// - Invalid price/size
    /// - Network/RPC errors (live trading)
    async fn place_limit_order_with_level(
        &self,
        side: OrderSide,
        price: f64,
        size: f64,
        grid_level_id: Option<u64>,
    ) -> TradingResult<String>;

    /// Cancel specific order by ID
    ///
    /// # Arguments
    /// * `order_id` - Order ID returned from place_limit_order_with_level
    ///
    /// # Safety
    /// For live trading with atomic swaps, this may be a no-op if order already executed
    async fn cancel_order(&self, order_id: &str) -> TradingResult<()>;

    /// Cancel all open orders
    ///
    /// # Returns
    /// Number of orders successfully cancelled
    ///
    /// # Warning
    /// Use sparingly! This cancels ALL orders including those with filled buys.
    /// Prefer selective cancellation via cancel_order() for grid trading.
    async fn cancel_all_orders(&self) -> TradingResult<usize>;

    /// Process price update and return filled order IDs
    ///
    /// # Arguments
    /// * `current_price` - Current market price in USD
    ///
    /// # Returns
    /// Vector of order IDs that were filled at this price
    ///
    /// # Implementation Notes
    /// - Paper trading: Simulates fills based on price crossing limit
    /// - Live trading: Queries on-chain state for fill confirmations
    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<String>>;

    /// Get count of currently open orders
    async fn open_order_count(&self) -> usize;

    // ═════════════════════════════════════════════════════════════════════════
    // RISK MANAGEMENT (Required)
    // ═════════════════════════════════════════════════════════════════════════

    /// Check if trading is allowed (circuit breaker + emergency shutdown)
    ///
    /// # Returns
    /// - `true` if orders can be placed
    /// - `false` if circuit breaker tripped or emergency shutdown active
    async fn is_trading_allowed(&self) -> bool;

    /// Get current engine health status
    ///
    /// Used for monitoring and alerting
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

    // ═════════════════════════════════════════════════════════════════════════
    // ADVANCED OPERATIONS (Optional - Future Extensions)
    // ═════════════════════════════════════════════════════════════════════════

    /// Place multiple orders in a single batch (optimized for gas/latency)
    ///
    /// # Future Extension (V4.2)
    /// Default implementation places orders sequentially.
    /// Live trading implementations can override for true batching.
    async fn place_batch_orders(
        &self,
        orders: Vec<BatchOrderRequest>,
    ) -> TradingResult<Vec<OrderPlacementResult>> {
        let mut results = Vec::with_capacity(orders.len());

        for order in orders {
            match self.place_limit_order_with_level(
                order.side,
                order.price,
                order.size,
                order.grid_level_id,
            ).await {
                Ok(order_id) => {
                    results.push(OrderPlacementResult::simple(order_id));
                }
                Err(e) => {
                    log::warn!("Batch order failed: {}", e);
                    // Continue with remaining orders
                }
            }
        }

        Ok(results)
    }

    /// Get detailed order information (for debugging/monitoring)
    ///
    /// # Future Extension (V4.2)
    /// Returns None by default. Implementations can override for rich order data.
    async fn get_order_details(&self, _order_id: &str) -> Option<Order> {
        None
    }

    /// Get estimated execution price for market conditions
    ///
    /// # Future Extension (V4.2)
    /// Used for slippage estimation and route optimization
    async fn estimate_execution_price(&self, _side: OrderSide, _size: f64) -> Option<f64> {
        None
    }

    /// Emergency shutdown - cancel all orders and stop trading
    ///
    /// # Future Extension (V4.3)
    /// For critical failures or market anomalies
    async fn emergency_shutdown(&self, _reason: &str) -> TradingResult<()> {
        log::error!("🚨 EMERGENCY SHUTDOWN - {}", _reason);
        self.cancel_all_orders().await?;
        Ok(())
    }
}

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
        // Engines
        PaperTradingEngine,
        RealTradingEngine,      // 🔥 NOW AVAILABLE!
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

        // Jupiter (🆕 V4.1)
        JupiterSwapClient,
        QuoteResponse,        // ✅ FIXED: Was JupiterQuote

        // Results
        TradingResult,
        OrderPlacementResult,
        EngineHealthStatus,
        
        // Real Trading (🔥 V4.1)
        RealTradingConfig,
        RealPerformanceStats,
        
        // Enhanced Metrics (📊 V4.1)
        EnhancedMetrics,
        
        // Adaptive Optimizer (🧠 V4.2)
        AdaptiveOptimizer,
        OptimizationResult,
        
        // MEV Protection (🛡️ V5.0) - NEW!
        MevProtectionManager,
        MevProtectionConfig,
        PriorityFeeOptimizer,
        SlippageGuardian,
        JitoClient,
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
        assert!(result.estimated_price.is_none());
    }

    #[test]
    fn test_order_placement_result_detailed() {
        let result = OrderPlacementResult::detailed(
            "ORDER-456".to_string(),
            "SIG-789".to_string(),
            200.50,
            0.10,
        );
        assert_eq!(result.order_id, "ORDER-456");
        assert_eq!(result.signature.unwrap(), "SIG-789");
        assert_eq!(result.estimated_price.unwrap(), 200.50);
        assert_eq!(result.estimated_fees.unwrap(), 0.10);
    }

    #[test]
    fn test_module_exports() {
        // Verify that all new exports are available
        use super::prelude::*;
        
        // This will compile if all exports are correct
        let _: Option<RealTradingConfig> = None;
        let _: Option<EnhancedMetrics> = None;  // 📊 V4.1 export test
        let _: Option<AdaptiveOptimizer> = None; // 🧠 V4.2 export test
        let _: Option<MevProtectionManager> = None; // 🛡️ V5.0 export test
    }
}
