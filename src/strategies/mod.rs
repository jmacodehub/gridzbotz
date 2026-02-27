// PROJECT FLASH V5.5 - STRATEGY ENGINE (Level-Crossing Edition)
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
//   - Stage 3 / Step 2:  notify_fill() -- FillEvent dispatched from GridBot
//   - Stage 3 / Step 6:  Signal::Buy / Sell carry level_id (Option<u64>)
//                        None  = grid init / full reposition
//                        Some  = specific grid level crossing detected
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
}

// =============================================================================
// SIGNAL STRUCTURE - UNIFIED CROSS-MODULE STANDARD
//
// Stage 3 / Step 6 change:
//   Signal::Buy and Signal::Sell now carry `level_id: Option<u64>`.
//
//   Semantics:
//     level_id = None      →  grid-wide action (init, full reposition)
//     level_id = Some(id)  →  price crossed the boundary of grid level `id`
//
//   All other strategies (RSI, Momentum, …) set level_id = None — the field
//   is grid-specific and ignored by the ConsensusEngine weighting logic.
// =============================================================================
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    StrongBuy {
        price:      f64,
        size:       f64,
        reason:     String,
        confidence: f64,
    },
    Buy {
        price:      f64,
        size:       f64,
        reason:     String,
        confidence: f64,
        /// Grid level that was crossed, or None for grid-wide actions.
        level_id:   Option<u64>,
    },
    Hold {
        reason: Option<String>,
    },
    Sell {
        price:      f64,
        size:       f64,
        reason:     String,
        confidence: f64,
        /// Grid level that was crossed, or None for grid-wide actions.
        level_id:   Option<u64>,
    },
    StrongSell {
        price:      f64,
        size:       f64,
        reason:     String,
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

    pub fn display(&self) -> String {
        match self {
            Signal::StrongBuy { price, reason, confidence, .. } => format!(
                "STRONG BUY @ ${:.4} | {} | {:.0}% conf",
                price, reason, confidence * 100.0
            ),
            Signal::Buy { price, reason, confidence, level_id, .. } => match level_id {
                Some(id) => format!(
                    "BUY crossing L{} @ ${:.4} | {} | {:.0}%",
                    id, price, reason, confidence * 100.0
                ),
                None => format!(
                    "BUY @ ${:.4} | {} | {:.0}%",
                    price, reason, confidence * 100.0
                ),
            },
            Signal::Hold { reason } => format!(
                "HOLD | {}",
                reason.clone().unwrap_or_else(|| "Neutral".into())
            ),
            Signal::Sell { price, reason, confidence, level_id, .. } => match level_id {
                Some(id) => format!(
                    "SELL crossing L{} @ ${:.4} | {} | {:.0}%",
                    id, price, reason, confidence * 100.0
                ),
                None => format!(
                    "SELL @ ${:.4} | {} | {:.0}%",
                    price, reason, confidence * 100.0
                ),
            },
            Signal::StrongSell { price, reason, confidence, .. } => format!(
                "STRONG SELL @ ${:.4} | {} | {:.0}%",
                price, reason, confidence * 100.0
            ),
        }
    }

    /// Returns the level_id if this is a level-crossing signal, else None.
    pub fn crossing_level_id(&self) -> Option<u64> {
        match self {
            Signal::Buy  { level_id, .. } => *level_id,
            Signal::Sell { level_id, .. } => *level_id,
            _ => None,
        }
    }

    /// True if this signal represents a grid-wide action (init / reposition).
    pub fn is_grid_action(&self) -> bool {
        matches!(
            self,
            Signal::Buy { level_id: None, .. } | Signal::Sell { level_id: None, .. }
        )
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

    /// Get reference to GridRebalancer strategy if present.
    /// Enables GridBot to notify the strategy about fills for adaptive learning.
    pub fn get_grid_rebalancer(&self) -> Option<&GridRebalancer> {
        for strategy in &self.strategies {
            if strategy.name().contains("Grid Rebalancer") {
                log::debug!("[StrategyMgr] Found GridRebalancer strategy");
                return None; // TODO: implement proper downcasting
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

    // Stage 3 / Step 6: level_id helpers
    #[test]
    fn test_crossing_level_id_helpers() {
        let grid_init = Signal::Buy {
            price: 100.0, size: 0.0,
            reason: "Grid init".into(), confidence: 1.0,
            level_id: None,
        };
        let crossing = Signal::Buy {
            price: 99.5, size: 0.1,
            reason: "Level 3 crossing".into(), confidence: 1.0,
            level_id: Some(3),
        };

        assert!(grid_init.is_grid_action());
        assert!(!crossing.is_grid_action());
        assert_eq!(grid_init.crossing_level_id(), None);
        assert_eq!(crossing.crossing_level_id(), Some(3));
    }

    // Stage 3 / Step 6: display includes level number for crossing signals
    #[test]
    fn test_display_shows_level_for_crossing() {
        let sig = Signal::Buy {
            price: 85.50, size: 0.1,
            reason: "buy crossing".into(), confidence: 1.0,
            level_id: Some(7),
        };
        assert!(sig.display().contains("L7"), "expected 'L7' in: {}", sig.display());
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
}
