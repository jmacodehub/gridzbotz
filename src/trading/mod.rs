//! ═══════════════════════════════════════════════════════════════════════════
//! Trading Module V5.1 - Security Hardening Complete!
//! ═══════════════════════════════════════════════════════════════════════════

pub use crate::config::Config;
pub use async_trait::async_trait;

// ═══════════════════════════════════════════════════════════════════════════
// Core Trading Modules
// ═══════════════════════════════════════════════════════════════════════════

pub mod price_feed;
pub mod pyth_price_feed;
pub mod pyth_http;
pub mod paper_trader;
pub mod grid_level;
pub mod executor;
pub mod trade;
pub mod feed_consensus;
pub mod redundant_feed;
pub mod jupiter_swap;
pub mod real_trader;
pub mod enhanced_metrics;
pub mod adaptive_optimizer;
pub mod mev_protection;

// ═══════════════════════════════════════════════════════════════════════════
// Security Modules (V5.1) 🔒 NEW!
// ═══════════════════════════════════════════════════════════════════════════

pub mod order_validator;
pub mod rpc_security;
pub mod rate_limiter;

// WebSocket feeds (optional feature)
#[cfg(feature = "websockets")]
pub mod pyth_websocket;
#[cfg(feature = "websockets")]
pub mod binance_ws;
#[cfg(feature = "websockets")]
pub mod pyth_lazer;

// ═══════════════════════════════════════════════════════════════════════════
// Exports
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

pub use grid_level::{
    GridLevel,
    GridLevelStatus,
    GridStateTracker,
};

pub use enhanced_metrics::EnhancedMetrics;

pub use adaptive_optimizer::{
    AdaptiveOptimizer,
    OptimizationResult,
};

pub use mev_protection::{
    MevProtectionManager,
    MevProtectionConfig,
    PriorityFeeOptimizer,
    PriorityFeeConfig,
    FeeRecommendation,
    SlippageGuardian,
    SlippageConfig,
    SlippageValidation,
    JitoClient,
    JitoConfig,
    JitoBundleStatus,
};

// Security Exports (V5.1) 🔒 - FIXED!
pub use order_validator::{
    OrderValidator,
    ValidationResult,
};

pub use rpc_security::{
    SecureRpcClient,
};

pub use rate_limiter::{
    TradeRateLimiter,
    RateLimitConfig,  // FIXED: Was RateLimiterConfig
};

pub use jupiter_swap::{
    JupiterSwapClient,
    QuoteResponse,
    SwapRequest,
    SwapResponse,
    WSOL_MINT,
    USDC_MINT,
};

pub use real_trader::{
    RealTradingEngine,
    RealTradingConfig,
    PerformanceStats as RealPerformanceStats,
};

pub use executor::{
    TransactionExecutor,
    ExecutorConfig,
    ExecutionStats,
};

pub use trade::Trade;

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

// ═══════════════════════════════════════════════════════════════════════════
// UNIFIED TRADING ENGINE TRAIT
// ═══════════════════════════════════════════════════════════════════════════

pub type TradingResult<T> = anyhow::Result<T>;

#[derive(Debug, Clone)]
pub struct OrderPlacementResult {
    pub order_id: String,
    pub signature: Option<String>,
    pub estimated_price: Option<f64>,
    pub estimated_fees: Option<f64>,
}

impl OrderPlacementResult {
    pub fn simple(order_id: String) -> Self {
        Self {
            order_id,
            signature: None,
            estimated_price: None,
            estimated_fees: None,
        }
    }

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

#[derive(Debug, Clone)]
pub struct BatchOrderRequest {
    pub side: OrderSide,
    pub price: f64,
    pub size: f64,
    pub grid_level_id: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct EngineHealthStatus {
    pub is_healthy: bool,
    pub trading_allowed: bool,
    pub circuit_breaker_active: bool,
    pub last_successful_trade: Option<i64>,
    pub error_rate: f64,
    pub message: String,
}

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
                }
            }
        }

        Ok(results)
    }

    async fn get_order_details(&self, _order_id: &str) -> Option<Order> {
        None
    }

    async fn estimate_execution_price(&self, _side: OrderSide, _size: f64) -> Option<f64> {
        None
    }

    async fn emergency_shutdown(&self, _reason: &str) -> TradingResult<()> {
        log::error!("🚨 EMERGENCY SHUTDOWN - {}", _reason);
        self.cancel_all_orders().await?;
        Ok(())
    }
}

pub async fn get_live_price(feed_id: &str) -> Option<f64> {
    let http = PythHttpFeed::new(vec![feed_id.to_string()]);
    if http.start().await.is_ok() {
        http.get_price(feed_id).await
    } else {
        None
    }
}

pub mod prelude {
    pub use super::{
        PaperTradingEngine,
        RealTradingEngine,
        TradingEngine,
        OrderSide,
        OrderStatus,
        OrderType,
        Order,
        GridStateTracker,
        GridLevel,
        GridLevelStatus,
        PriceFeed,
        FeedMode,
        JupiterSwapClient,
        QuoteResponse,
        TradingResult,
        OrderPlacementResult,
        EngineHealthStatus,
        RealTradingConfig,
        RealPerformanceStats,
        EnhancedMetrics,
        AdaptiveOptimizer,
        OptimizationResult,
        MevProtectionManager,
        MevProtectionConfig,
        PriorityFeeOptimizer,
        SlippageGuardian,
        JitoClient,
        OrderValidator,
        ValidationResult,
        SecureRpcClient,
        TradeRateLimiter,
        RateLimitConfig,
    };
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_security_exports() {
        use super::prelude::*;
        
        let _: Option<OrderValidator> = None;
        let _: Option<SecureRpcClient> = None;
        let _: Option<TradeRateLimiter> = None;
    }
}
