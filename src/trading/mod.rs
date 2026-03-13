//! ═════════════════════════════════════════════════════════════════
//! Trading Module V5.9 - Jupiter-Powered Real Trading Engine (Consolidated)
//!
//! Architecture:
//! - Unified Trading Interface: Generic trait for paper and live trading
//! - Engine Factory: Config-driven engine selection (paper ↔ live)
//! - Paper Trading: Risk-free backtesting and simulation
//! - Grid State Machine: Order lifecycle tracking with buy/sell pairing
//! - Real Trading: ✅ ENABLED - Live execution with Jupiter swaps!
//! - Jupiter Client: V4.0 — Production client from src/dex/ with API key support
//! - Price Feeds: Multiple sources with redundancy and consensus
//! - Transaction Executor: Solana transaction building and signing
//! - Enhanced Metrics: Trade-level analytics and performance tracking
//! - Adaptive Optimizer: Self-learning grid spacing and position sizing
//! - Wallet Utils: Shared on-chain balance query (single-bot + fleet)
//!
//! V5.9 CHANGES (PR #109 — Dynamic Priority Fee Sources):
//! ✅ rpc_fee_source.rs — RpcFeeSource: FeeDataSource via getRecentPrioritizationFees
//!    Passes JUP6Lk + SOL + USDC account keys for Jupiter local fee market accuracy.
//! ✅ helius_fee_source.rs — HeliusFeeSource: FeeDataSource via getPriorityFeeEstimate
//!    Helius V2 algo: max(global_percentile, per_account_percentile). Helius only.
//! ✅ PriorityFeeConfig gains `source` field: "rpc" | "helius" (serde default = "rpc")
//! ✅ smoke_test.rs wired: live fee estimated at stage [3/5], hardcoded 10_000 removed
//!
//! V5.8 CHANGES (PR #86 — Multi-Bot Orchestrator):
//! ✅ wallet_utils.rs — fetch_wallet_balances_for_orchestrator() extracted from main.rs
//!    Shared between main.rs (single-bot) and orchestrator.rs (fleet mode)
//!
//! V5.4 CHANGES (Mar 2026 — PR #72):
//! ✅ price_feed_utils.rs — fetch_pyth_price() with 3x retry + confidence check
//! ✅ engine.rs V2 — EngineParams, fees/slippage for paper, keystore for live
//! ✅ EngineParams exported in prelude for main.rs wiring
//!
//! V5.4 CHANGES (Mar 2026 — PR #71):
//! ✅ engine.rs added — create_engine() factory for config-driven mode selection
//!    Reads bot.execution_mode → returns Arc<dyn TradingEngine>
//!    Paper: instant, no network | Live: Pyth price + wallet validation
//! ✅ engine_mode_label() helper for logging/metrics
//! ✅ Prelude updated with factory exports
//!
//! V5.3.1 CHANGES (Mar 2026 — export cleanup):
//! ✅ Removed phantom Jupiter client exports (JupiterConfig, private types)
//! ✅ Added WSOL_MINT as local const alias to SOL_MINT (backwards compat)
//! ✅ Only export what's actually public: JupiterClient, SOL_MINT, USDC_MINT
//!
//! V5.3 CHANGES (Mar 2026 — production Jupiter client):
//! ✅ Old src/trading/jupiter_client.rs stub removed
//! ✅ Production JupiterClient V4.0 wired from src/dex/
//! ✅ Full API key support, proper error handling, dynamic slippage
//!
//! V5.2.1 CHANGES (Feb 2026 — per-level analytics):
//! ✅ FillEvent gains level_id: Option<u64>            — grid level ID
//! ✅ FillEvent gains distance_from_mid_pct: Option<f64> — % from mid at fill
//! ✅ Builder methods: .with_level() / .with_distance_from_mid()
//! ✅ All existing call sites unaffected (new fields default to None)
//!
//! V5.2 CHANGES (Stage 3 — Feb 2026):
//! ✅ FillEvent added — data carrier for confirmed order fills
//!    Fan-out to strategies via StrategyManager::notify_fill()
//!
//! V5.1 CHANGES (Feb 2026):
//! ✅ jupiter_swap.rs removed — consolidated into jupiter_client.rs
//! ✅ JupiterClient is now the single canonical implementation
//! ✅ VersionedTransaction preserved end-to-end (ALTs no longer dropped)
//! ✅ executor.execute_versioned() wired for Jupiter swaps
//! ✅ keystore.sign_versioned_transaction() added
//!
//! March 2026 - V5.9 DYNAMIC FEES ⚡
//! ═════════════════════════════════════════════════════════════════

pub use crate::config::Config;

// Re-export async_trait for trait implementations
pub use async_trait::async_trait;

// ═════════════════════════════════════════════════════════════════
// Core Trading Modules
// ═════════════════════════════════════════════════════════════════

pub mod price_feed;
pub mod pyth_price_feed;
pub mod pyth_http;
pub mod paper_trader;
pub mod grid_level;             // V4.0: Grid state machine
pub mod executor;               // Transaction executor
pub mod trade;                  // Trade data structures
pub mod feed_consensus;         // Feed consensus logic
pub mod redundant_feed;         // Redundant price feeds
pub mod real_trader;            // 🔥 ENABLED - Phase 5 Complete!
pub mod enhanced_metrics;       // 📊 V4.1: Enhanced analytics tracking
pub mod adaptive_optimizer;     // 🧠 V4.2: Self-learning optimizer
pub mod engine;                 // 🏭 V5.4: Config-driven engine factory
pub mod price_feed_utils;       // 📡 V5.4: Pyth HTTP price fetching with retry (PR #72)
pub mod priority_fee_estimator; // ⚡ Dynamic priority fee estimation (EXEC-4)
pub mod wallet_utils;           // 💰 V5.8: Shared on-chain balance query (PR #86)
pub mod rpc_fee_source;         // ⚡ V5.9: Standard getRecentPrioritizationFees (PR #109)
pub mod helius_fee_source;      // ⚡ V5.9: Helius getPriorityFeeEstimate V2 (PR #109)

// WebSocket feeds (optional feature)
#[cfg(feature = "websockets")]
pub mod pyth_websocket;
#[cfg(feature = "websockets")]
pub mod binance_ws;
#[cfg(feature = "websockets")]
pub mod pyth_lazer;

// ═════════════════════════════════════════════════════════════════
// Wallet Utils Export (V5.8 — PR #86) 💰
// ═════════════════════════════════════════════════════════════════

pub use wallet_utils::fetch_wallet_balances_for_orchestrator;

// ═════════════════════════════════════════════════════════════════
// Engine Factory Exports (V5.4 — PR #72) 🏭
// ═════════════════════════════════════════════════════════════════

pub use engine::{
    create_engine,
    engine_mode_label,
    EngineParams,
};

// ═════════════════════════════════════════════════════════════════
// Paper Trading Exports
// ═════════════════════════════════════════════════════════════════

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

// ═════════════════════════════════════════════════════════════════
// Grid Level State Machine Exports (V4.0)
// ═════════════════════════════════════════════════════════════════

pub use grid_level::{
    GridLevel,
    GridLevelStatus,
    GridStateTracker,
};

// ═════════════════════════════════════════════════════════════════
// Enhanced Metrics Exports (V4.1) 📊
// ═════════════════════════════════════════════════════════════════

pub use enhanced_metrics::EnhancedMetrics;

// ═════════════════════════════════════════════════════════════════
// Adaptive Optimizer Exports (V4.2) 🧠
// ═════════════════════════════════════════════════════════════════

pub use adaptive_optimizer::{
    AdaptiveOptimizer,
    OptimizationResult,
};

// ═════════════════════════════════════════════════════════════════
// Jupiter Client Exports (V5.3.1 / Mar 2026) 🪐 — Production from src/dex/
// ═════════════════════════════════════════════════════════════════

pub use crate::dex::jupiter_client::{
    JupiterClient,
    SOL_MINT,
    USDC_MINT,
};

// WSOL_MINT backwards-compat alias (SOL and WSOL are the same on Solana)
pub const WSOL_MINT: &str = SOL_MINT;

// ═════════════════════════════════════════════════════════════════
// Real Trading Exports (🔥 ENABLED - Phase 5!)
// ═════════════════════════════════════════════════════════════════

pub use real_trader::{
    RealTradingEngine,
    RealTradingConfig,
    PerformanceStats as RealPerformanceStats,
};

// ═════════════════════════════════════════════════════════════════
// Transaction Executor Exports
// ═════════════════════════════════════════════════════════════════

pub use executor::{
    TransactionExecutor,
    ExecutorConfig,
    ExecutionStats,
};

pub use trade::Trade;

// ═════════════════════════════════════════════════════════════════
// Priority Fee Estimator Exports (⚡ EXEC-4)
// ═════════════════════════════════════════════════════════════════

pub use priority_fee_estimator::{PriorityFeeEstimator, FeeDataSource, MockFeeSource};

// ═════════════════════════════════════════════════════════════════
// Fee Source Exports (⚡ V5.9 — PR #109)
// ═════════════════════════════════════════════════════════════════

pub use rpc_fee_source::RpcFeeSource;
pub use helius_fee_source::HeliusFeeSource;

// ═════════════════════════════════════════════════════════════════
// Price Feed Exports
// ═════════════════════════════════════════════════════════════════

pub use price_feed::{PriceFeed, PriceFeedMetrics, FeedMode};
pub use pyth_http::{PythHttpFeed, PriceUpdate as HttpPriceUpdate, feed_ids as http_feed_ids};
pub use pyth_price_feed::PythPriceFeed;

#[cfg(feature = "websockets")]
pub use pyth_websocket::{PythWebSocketFeed, PriceUpdate as WsPriceUpdate};

// ═════════════════════════════════════════════════════════════════
// Conditional Type Aliases (WebSocket vs HTTP)
// ═════════════════════════════════════════════════════════════════

#[cfg(feature = "websockets")]
pub type LivePriceUpdate = WsPriceUpdate;
#[cfg(not(feature = "websockets"))]
pub type LivePriceUpdate = HttpPriceUpdate;

#[cfg(feature = "websockets")]
pub use pyth_http::feed_ids as live_feed_ids;
#[cfg(not(feature = "websockets"))]
pub use http_feed_ids as live_feed_ids;

// ═════════════════════════════════════════════════════════════════
// UNIFIED TRADING ENGINE TRAIT (V4.1) 🚀
// ═════════════════════════════════════════════════════════════════

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

// ═════════════════════════════════════════════════════════════════
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
// ═════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct FillEvent {
    pub order_id:   String,
    pub side:       OrderSide,
    pub fill_price: f64,
    pub fill_size:  f64,
    pub fee_usdc:   f64,
    pub pnl:        Option<f64>,
    pub timestamp:  i64,
    pub level_id:              Option<u64>,
    pub distance_from_mid_pct: Option<f64>,
}

impl FillEvent {
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

    #[inline]
    pub fn with_level(mut self, level_id: u64) -> Self {
        self.level_id = Some(level_id);
        self
    }

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

    async fn process_price_update(&self, current_price: f64) -> TradingResult<Vec<FillEvent>>;

    async fn open_order_count(&self) -> usize;
    async fn is_trading_allowed(&self) -> bool;

    async fn get_wallet(&self) -> VirtualWallet;
    async fn get_performance_stats(&self) -> PaperPerformanceStats;

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

// ═════════════════════════════════════════════════════════════════
// Helper Functions
// ═════════════════════════════════════════════════════════════════

pub async fn get_live_price(feed_id: &str) -> Option<f64> {
    let http = PythHttpFeed::new(vec![feed_id.to_string()]);
    if http.start().await.is_ok() {
        http.get_price(feed_id).await
    } else {
        None
    }
}

// ═════════════════════════════════════════════════════════════════
// Re-exports for Convenience
// ═════════════════════════════════════════════════════════════════

pub mod prelude {
    pub use super::{
        // Engine Factory (V5.4 — PR #72) 🏭
        create_engine,
        engine_mode_label,
        EngineParams,

        // Wallet Utils (V5.8 — PR #86) 💰
        fetch_wallet_balances_for_orchestrator,

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

        // Jupiter V5.3.1 — production from src/dex/ (cleaned exports)
        JupiterClient,
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

        // Priority Fee Estimator (⚡ EXEC-4)
        PriorityFeeEstimator,
        FeeDataSource,

        // Fee Sources (⚡ V5.9 — PR #109)
        RpcFeeSource,
        HeliusFeeSource,
    };
}

// ═════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════

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
            "ORDER-BUY-001", OrderSide::Buy, 142.50, 0.1, 0.0025, Some(0.05), 1_700_000_000,
        );
        assert_eq!(fill.order_id, "ORDER-BUY-001");
        assert_eq!(fill.fill_price, 142.50);
        assert!(fill.level_id.is_none());
        assert!(fill.distance_from_mid_pct.is_none());
    }

    #[test]
    fn test_fill_event_builder_chain() {
        let mid = 155.00_f64;
        let price = 153.14_f64;
        let dist = (price - mid) / mid * 100.0;
        let fill = FillEvent::new(
            "ORDER-BUY-003", OrderSide::Buy, price, 0.2, 0.003, Some(1.85), 1_700_000_200,
        )
        .with_level(3)
        .with_distance_from_mid(dist);
        assert_eq!(fill.level_id, Some(3));
        assert!(fill.distance_from_mid_pct.unwrap() < 0.0);
    }

    #[test]
    fn test_wsol_mint_alias() {
        assert_eq!(WSOL_MINT, SOL_MINT);
    }

    #[test]
    fn test_module_exports() {
        use super::prelude::*;
        let _: Option<RealTradingConfig> = None;
        let _: Option<EnhancedMetrics>   = None;
        let _: Option<AdaptiveOptimizer> = None;
        let _: Option<JupiterClient>     = None;
        let _: Option<FillEvent>         = None;
        let _sol: &str  = SOL_MINT;
        let _wsol: &str = WSOL_MINT;
        let _usdc: &str = USDC_MINT;
    }
}
