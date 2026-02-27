// PROJECT FLASH V5.5 - STRATEGY ENGINE (Crossing Detection Edition)
// =============================================================================
//
// Purpose:
//   Asynchronous multi-strategy manager for modular trading orchestration.
//   Ready for Phase 4 fusion layer & signal consensus integration.
//
// Highlights:
//   - Clean async execution for all strategy modules.
//   - ConsensusEngine-ready architecture for signal weighting.
//   - Unified `Signal` standard for all decision agents.
//   - Derived lightweight stats for diagnostic analytics.
//   - Monitor-friendly volatility access for live dashboards.
//   - V4.0 Grid State Machine compatible (added missing methods)
//   - GridRebalancer access for fill notifications
//   - Stage 3 / Step 2: notify_fill() -- FillEvent dispatched from GridBot
//   - Stage 3 / Option B Step 1: level_id field on Signal::Buy/Sell
//     None   = grid infrastructure action (init / reposition)
//     Some(n) = price crossed grid level n boundary
//   - Stage 3 / Step 6: get_grid_rebalancer_mut() added so GridBot can
//     push level snapshots and anchor into GridRebalancer after placement.
// =============================================================================

use anyhow::Result;
use async_trait::async_trait;
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt;

// Stage 3 / Step 2: FillEvent dispatched from GridBot via notify_fill()
use crate::trading::FillEvent;

pub mod arbitrage;
pub mod consensus;
pub mod grid_rebalancer;
pub mod shared;

pub use arbitrage::*;
pub use consensus::*;
pub use grid_rebalancer::*;
pub use shared::*;

// =============================================================================
// STRATEGY TRAIT - ASYNC AND CONTEXT-AWARE
// =============================================================================
#[async_trait]
pub trait Strategy: Send + Sync + 'static {
    fn name(&self) -> &str;
    async fn analyze(&mut self, price: f64, timestamp: i64) -> Result<Signal>;
    fn stats(&self) -> StrategyStats;
    fn reset(&mut self);
    fn attach_analytics(&mut self, _ctx: AnalyticsContext) {}
    fn is_enabled(&self) -> bool {
        true
    }
    fn last_signal(&self) -> Option<Signal> {
        None
    }
    async fn initialize_at_price(&mut self, _price: f64) -> Result<()> {
        Ok(())
    }
    /// Downcast helper: returns a mutable reference to GridRebalancer if
    /// this strategy IS a GridRebalancer, otherwise None.
    fn as_grid_rebalancer_mut(&mut self) -> Option<&mut GridRebalancer> {
        None
    }
}

// =============================================================================
// SIGNAL STRUCTURE - UNIFIED CROSS-MODULE STANDARD
//
// Stage 3 / Option B: level_id added to Buy and Sell variants.
//
// Semantics:
//   level_id = None    → infrastructure signal (grid init or full reposition)
//   level_id = Some(n) → price crossed the boundary of grid level n
//
// All existing call sites that construct Signal::Buy or Signal::Sell
// must pass `level_id: None` unless they are emitting a crossing signal.
// The compiler enforces this at every construction site.
// =============================================================================
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    StrongBuy {
        price: f64,
        size: f64,
        reason: String,
        confidence: f64,
    },
    Buy {
        price: f64,
        size: f64,
        reason: String,
        confidence: f64,
        /// Which grid level triggered this signal.
        /// None  = grid-infrastructure (init / reposition)
        /// Some(id) = price crossed level `id`
        level_id: Option<u64>,
    },
    Hold {
        reason: Option<String>,
    },
    Sell {
        price: f64,
        size: f64,
        reason: String,
        confidence: f64,
        /// Which grid level triggered this signal.
        /// None  = grid-infrastructure (init / reposition)
        /// Some(id) = price crossed level `id`
        level_id: Option<u64>,
    },
    StrongSell {
        price: f64,
        size: f64,
        reason: String,
        confidence: f64,
    },
}

impl Signal {
    pub fn to_order_side(&self) -> Option<crate::trading::OrderSide> {
        match self {
            Signal::StrongBuy { .. } | Signal::Buy { .. } => Some(crate::trading::OrderSide::Buy),
            Signal::StrongSell { .. } | Signal::Sell { .. } => {
                Some(crate::trading::OrderSide::Sell)
            }
            Signal::Hold { .. } => None,
        }
    }

    pub fn is_bullish(&self) -> bool {
        matches!(self, Signal::Buy { .. } | Signal::StrongBuy { .. })
    }

    pub fn is_bearish(&self) -> bool {
        matches!(self, Signal::Sell { .. } | Signal::StrongSell { .. })
    }

    pub fn strength(&self) -> f64 {
        match self {
            Signal::StrongBuy { confidence, .. } | Signal::StrongSell { confidence, .. } => {
                0.5 + confidence * 0.5
            }
            Signal::Buy { confidence, .. } | Signal::Sell { confidence, .. } => {
                0.25 + confidence * 0.25
            }
            Signal::Hold { .. } => 0.0,
        }
    }

    /// Return the level_id carried by a Buy or Sell signal, if any.
    pub fn level_id(&self) -> Option<u64> {
        match self {
            Signal::Buy  { level_id, .. } => *level_id,
            Signal::Sell { level_id, .. } => *level_id,
            _ => None,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Signal::StrongBuy { price, reason, confidence, .. } => format!(
                "STRONG BUY @ ${:.4} | {} | {:.0}% conf",
                price, reason, confidence * 100.0
            ),
            Signal::Buy { price, reason, confidence, level_id, .. } => {
                if let Some(id) = level_id {
                    format!("BUY L{} @ ${:.4} | {} | {:.0}%", id, price, reason, confidence * 100.0)
                } else {
                    format!("BUY @ ${:.4} | {} | {:.0}%", price, reason, confidence * 100.0)
                }
            }
            Signal::Hold { reason } => format!(
                "HOLD | {}",
                reason.clone().unwrap_or_else(|| "Neutral".into())
            ),
            Signal::Sell { price, reason, confidence, level_id, .. } => {
                if let Some(id) = level_id {
                    format!("SELL L{} @ ${:.4} | {} | {:.0}%", id, price, reason, confidence * 100.0)
                } else {
                    format!("SELL @ ${:.4} | {} | {:.0}%", price, reason, confidence * 100.0)
                }
            }
            Signal::StrongSell { price, reason, confidence, .. } => format!(
                "STRONG SELL @ ${:.4} | {} | {:.0}%",
                price, reason, confidence * 100.0
            ),
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

// =============================================================================
// STRATEGY STATS - LIGHTWEIGHT PERFORMANCE METRICS
// =============================================================================
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyStats {
    pub signals_generated:    u64,
    pub buy_signals:          u64,
    pub sell_signals:         u64,
    pub hold_signals:         u64,
    pub active_trades:        u64,
    pub total_pnl:            f64,
    pub win_rate:             f64,
    pub sharpe:               f64,
    pub rebalances_executed:  u64,
}

impl StrategyStats {
    pub fn record_signal(&mut self, signal: &Signal) {
        self.signals_generated += 1;
        match signal {
            Signal::Buy { .. } | Signal::StrongBuy { .. }   => self.buy_signals  += 1,
            Signal::Sell { .. } | Signal::StrongSell { .. } => self.sell_signals += 1,
            Signal::Hold { .. }                             => self.hold_signals += 1,
        }
    }
}

// =============================================================================
// STRATEGY MANAGER - ASYNC CONSENSUS ORCHESTRATOR
// =============================================================================
pub struct StrategyManager {
    pub strategies: Vec<Box<dyn Strategy>>,
    pub engine:     ConsensusEngine,
    pub context:    AnalyticsContext,
    /// Running count of fills dispatched via notify_fill().
    /// Incremented on every fill for log correlation.
    fill_count:     u64,
}

impl std::fmt::Debug for StrategyManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StrategyManager with {} strategies", self.strategies.len())
    }
}

impl StrategyManager {
    pub fn new(ctx: AnalyticsContext) -> Self {
        info!("[StrategyMgr] V5.5 initialized");
        Self {
            strategies: Vec::new(),
            engine:     ConsensusEngine::new(ConsensusMode::default()),
            context:    ctx,
            fill_count: 0,
        }
    }

    pub fn add_strategy<S: Strategy + 'static>(&mut self, strategy: S) {
        let mut boxed = Box::new(strategy);
        boxed.attach_analytics(self.context.clone());
        info!("[StrategyMgr] Attached: {}", boxed.name());
        self.strategies.push(boxed);
    }

    pub async fn analyze_all(&mut self, price: f64, ts: i64) -> Result<Signal> {
        use futures::stream::{FuturesUnordered, StreamExt};

        if self.strategies.is_empty() {
            return Ok(Signal::Hold {
                reason: Some("no strategies loaded".into()),
            });
        }

        let mut results = Vec::new();
        let mut futs = FuturesUnordered::new();
        for s in &mut self.strategies {
            futs.push(s.analyze(price, ts));
        }

        while let Some(res) = futs.next().await {
            if let Ok(sig) = res {
                results.push(sig);
            }
        }

        Ok(self.engine.resolve(&results))
    }

    pub async fn initialize_all_at_price(&mut self, price: f64) -> Result<()> {
        for strategy in &mut self.strategies {
            strategy.initialize_at_price(price).await?;
        }
        Ok(())
    }

    pub fn get_current_volatility(&self) -> Option<f64> {
        self.context.get_current_volatility()
    }

    pub fn display_stats(&self) {
        println!("\nStrategy Performance (V5.5):");
        for (i, strategy) in self.strategies.iter().enumerate() {
            let stats = strategy.stats();
            println!("  Strategy {} ({}): {} signals",
                i + 1, strategy.name(), stats.signals_generated);
            println!("    Buy: {}, Sell: {}, Hold: {}",
                stats.buy_signals, stats.sell_signals, stats.hold_signals);
            if stats.rebalances_executed > 0 {
                println!("    Rebalances: {}", stats.rebalances_executed);
            }
            if stats.total_pnl != 0.0 {
                println!("    P&L: ${:.2} | Win Rate: {:.1}% | Sharpe: {:.2}",
                    stats.total_pnl, stats.win_rate, stats.sharpe);
            }
        }
    }

    // =========================================================================
    // Stage 3 / Step 2 -- FILL NOTIFICATION
    // Called by GridBot for every filled order so strategies can react.
    // Logs full fill context immediately; strategy dispatch is wired in Step 3.
    // =========================================================================

    /// Notify all strategies about a completed fill.
    ///
    /// Called by `GridBot` immediately after `process_price_update` returns
    /// a non-empty `Vec<FillEvent>`.  Each call increments `fill_count` so
    /// log lines carry a stable per-session sequence number.
    ///
    /// Stage 3 / Step 3 will fan this out to individual strategy `on_fill()`
    /// hooks; the TODO below is the only planned change at that point.
    pub async fn notify_fill(&mut self, fill: &FillEvent) {
        self.fill_count += 1;
        info!(
            "[StrategyMgr] Fill #{}: {:?} {} @ ${:.4} | size={:.4} SOL | fee=${:.4} | level={:?}",
            self.fill_count,
            fill.side,
            fill.order_id,
            fill.price,
            fill.size,
            fill.fee,
            fill.grid_level_id,
        );
        // TODO Stage 3 / Step 3: fan out to strategy on_fill() hooks:
        // for strategy in &mut self.strategies {
        //     strategy.on_fill(fill).await;
        // }
    }

    // =========================================================================
    // V5.5 GRID REBALANCER ACCESS
    // =========================================================================

    /// Get immutable reference to GridRebalancer strategy if present.
    #[allow(unused)]
    pub fn get_grid_rebalancer(&self) -> Option<&GridRebalancer> {
        // TODO: implement proper downcasting via Any
        None
    }

    /// Get mutable reference to GridRebalancer strategy if present.
    ///
    /// Used by `GridBot::sync_levels_to_strategy()` to push level snapshots
    /// and anchor price into the strategy after every grid placement.
    ///
    /// Uses the `as_grid_rebalancer_mut()` hook on the Strategy trait so the
    /// box can be downcast without requiring `std::any::Any`.
    pub fn get_grid_rebalancer_mut(&mut self) -> Option<&mut GridRebalancer> {
        for strategy in &mut self.strategies {
            if let Some(r) = strategy.as_grid_rebalancer_mut() {
                return Some(r);
            }
        }
        None
    }
}

// =============================================================================
// TESTS
// =============================================================================
#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::grid_rebalancer::{GridRebalancer, GridRebalancerConfig};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_manager_consensus_integration() {
        let ctx = AnalyticsContext::default();
        let mut mgr = StrategyManager::new(ctx);
        mgr.engine.mode = ConsensusMode::MajorityVote;

        mgr.add_strategy(GridRebalancer::new(GridRebalancerConfig::default()).unwrap());

        let sig = mgr.analyze_all(100.0, 1).await.unwrap();
        assert!(matches!(
            sig,
            Signal::Buy { .. } | Signal::Hold { .. } | Signal::Sell { .. }
        ));
    }

    #[test]
    fn test_signal_strength_v55() {
        let strong_buy = Signal::StrongBuy {
            price: 100.0, size: 1.0,
            reason: "test".into(), confidence: 0.9,
        };
        let buy = Signal::Buy {
            price: 100.0, size: 1.0,
            reason: "test".into(), confidence: 0.7,
            level_id: None,
        };
        let hold = Signal::Hold { reason: Some("test".into()) };

        assert!(strong_buy.strength() > buy.strength());
        assert!(buy.strength() > hold.strength());
        assert_eq!(hold.strength(), 0.0);
    }

    #[test]
    fn test_signal_level_id_accessor() {
        let infra = Signal::Buy {
            price: 100.0, size: 0.0,
            reason: "Grid init".into(), confidence: 1.0,
            level_id: None,
        };
        assert_eq!(infra.level_id(), None);

        let crossing = Signal::Sell {
            price: 102.0, size: 0.1,
            reason: "Level 5 sell crossing".into(), confidence: 1.0,
            level_id: Some(5),
        };
        assert_eq!(crossing.level_id(), Some(5));
    }

    #[test]
    fn test_signal_display_includes_level() {
        let sig = Signal::Buy {
            price: 85.5, size: 0.1,
            reason: "buy crossing".into(), confidence: 1.0,
            level_id: Some(3),
        };
        assert!(sig.display().contains("L3"), "display should include level id");
    }

    // Stage 3 / Step 2: verify notify_fill increments fill_count
    #[tokio::test]
    async fn test_notify_fill_increments_counter() {
        use crate::trading::{FillEvent, OrderSide};

        let ctx = AnalyticsContext::default();
        let mut mgr = StrategyManager::new(ctx);

        assert_eq!(mgr.fill_count, 0);

        let fill = FillEvent::new(
            "ORDER-000001-L2".to_string(),
            OrderSide::Buy,
            150.0, 0.1, 0.003, None,
            1_700_000_000,
        );

        mgr.notify_fill(&fill).await;
        assert_eq!(mgr.fill_count, 1);

        mgr.notify_fill(&fill).await;
        assert_eq!(mgr.fill_count, 2);
    }

    #[test]
    fn test_get_grid_rebalancer_mut_returns_some() {
        let ctx = AnalyticsContext::default();
        let mut mgr = StrategyManager::new(ctx);
        mgr.add_strategy(GridRebalancer::new(GridRebalancerConfig::default()).unwrap());
        assert!(mgr.get_grid_rebalancer_mut().is_some(),
            "get_grid_rebalancer_mut() must find the GridRebalancer we just added");
    }
}
