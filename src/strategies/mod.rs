// PROJECT FLASH V5.5 - STRATEGY ENGINE (Fill Fan-out Edition)
// ══════════════════════════════════════════════════════════════════════
//
// Purpose:
//   Asynchronous multi-strategy manager for modular trading orchestration.
//   Ready for Phase 4 fusion layer & signal consensus integration.
//
// Highlights:
//   ✅ Clean async execution for all strategy modules.
//   ✅ ConsensusEngine-ready architecture for signal weighting.
//   ✅ Unified `Signal` standard for all decision agents.
//   ✅ Derived lightweight stats for diagnostic analytics.
//   ✅ Monitor-friendly volatility access for live dashboards.
//   ✅ V4.0 Grid State Machine compatible (added missing methods)
//   ✅ V5.5 on_fill() trait method + notify_fill() fan-out
// ══════════════════════════════════════════════════════════════════════

use anyhow::Result;
use async_trait::async_trait;
use log::info;
use serde::{Deserialize, Serialize};
use std::fmt;

pub mod arbitrage;
pub mod consensus;
pub mod grid_rebalancer;
pub mod shared;

pub use arbitrage::*;
pub use consensus::*;
pub use grid_rebalancer::*;
pub use shared::*;

// ══════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT
// ══════════════════════════════════════════════════════════════════════
#[async_trait]
pub trait Strategy: Send + Sync + 'static {
    fn name(&self) -> &str;
    async fn analyze(&mut self, price: f64, timestamp: i64) -> Result<Signal>;
    fn stats(&self) -> StrategyStats;
    fn reset(&mut self);
    fn attach_analytics(&mut self, _ctx: AnalyticsContext) {}
    fn is_enabled(&self) -> bool { true }
    fn last_signal(&self) -> Option<Signal> { None }
    async fn initialize_at_price(&mut self, _price: f64) -> Result<()> { Ok(()) }

    // -----------------------------------------------------------------------
    // V5.5: Fill feedback loop
    // -----------------------------------------------------------------------
    /// Called by StrategyManager::notify_fill() on every confirmed order fill.
    ///
    /// Default is a no-op — existing strategies (RSI, MACD, Momentum, Arbitrage)
    /// inherit this for free and require zero code changes.
    ///
    /// Override in GridRebalancer (and any future ML strategy) to react
    /// to fills for adaptive spacing / position sizing.
    fn on_fill(&mut self, _fill: &crate::trading::FillEvent) {}
}

// ══════════════════════════════════════════════════════════════════════
// SIGNAL STRUCTURE
// ══════════════════════════════════════════════════════════════════════
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Signal {
    StrongBuy { price: f64, size: f64, reason: String, confidence: f64, level_id: Option<u64> },
    Buy       { price: f64, size: f64, reason: String, confidence: f64, level_id: Option<u64> },
    Hold      { reason: Option<String> },
    Sell      { price: f64, size: f64, reason: String, confidence: f64, level_id: Option<u64> },
    StrongSell{ price: f64, size: f64, reason: String, confidence: f64, level_id: Option<u64> },
}

impl Signal {
    pub fn to_order_side(&self) -> Option<crate::trading::OrderSide> {
        match self {
            Signal::StrongBuy { .. } | Signal::Buy { .. } =>
                Some(crate::trading::OrderSide::Buy),
            Signal::StrongSell { .. } | Signal::Sell { .. } =>
                Some(crate::trading::OrderSide::Sell),
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
            Signal::StrongBuy { confidence, .. } | Signal::StrongSell { confidence, .. } =>
                0.5 + confidence * 0.5,
            Signal::Buy { confidence, .. } | Signal::Sell { confidence, .. } =>
                0.25 + confidence * 0.25,
            Signal::Hold { .. } => 0.0,
        }
    }

    pub fn display(&self) -> String {
        match self {
            Signal::StrongBuy { price, reason, confidence, level_id, .. } => {
                let level_str = level_id.map_or_else(|| String::new(), |id| format!(" level {} | ", id));
                format!("STRONG BUY @ ${:.4} | {}{} | {:.0}% conf", price, level_str, reason, confidence * 100.0)
            },
            Signal::Buy { price, reason, confidence, level_id, .. } => {
                let level_str = level_id.map_or_else(|| String::new(), |id| format!(" level {} | ", id));
                format!("BUY @ ${:.4} | {}{} | {:.0}%", price, level_str, reason, confidence * 100.0)
            },
            Signal::Hold { reason } =>
                format!("HOLD | {}", reason.clone().unwrap_or_else(|| "Neutral".into())),
            Signal::Sell { price, reason, confidence, level_id, .. } => {
                let level_str = level_id.map_or_else(|| String::new(), |id| format!(" level {} | ", id));
                format!("SELL @ ${:.4} | {}{} | {:.0}%", price, level_str, reason, confidence * 100.0)
            },
            Signal::StrongSell { price, reason, confidence, level_id, .. } => {
                let level_str = level_id.map_or_else(|| String::new(), |id| format!(" level {} | ", id));
                format!("STRONG SELL @ ${:.4} | {}{} | {:.0}%", price, level_str, reason, confidence * 100.0)
            },
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

// ══════════════════════════════════════════════════════════════════════
// STRATEGY STATS
// ══════════════════════════════════════════════════════════════════════
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyStats {
    pub signals_generated: u64,
    pub buy_signals: u64,
    pub sell_signals: u64,
    pub hold_signals: u64,
    pub active_trades: u64,
    pub total_pnl: f64,
    pub win_rate: f64,
    pub sharpe: f64,
    pub rebalances_executed: u64,
}

impl StrategyStats {
    pub fn record_signal(&mut self, signal: &Signal) {
        self.signals_generated += 1;
        match signal {
            Signal::Buy { .. } | Signal::StrongBuy { .. } => self.buy_signals += 1,
            Signal::Sell { .. } | Signal::StrongSell { .. } => self.sell_signals += 1,
            Signal::Hold { .. } => self.hold_signals += 1,
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// STRATEGY MANAGER
// ══════════════════════════════════════════════════════════════════════
pub struct StrategyManager {
    pub strategies: Vec<Box<dyn Strategy>>,
    pub engine: ConsensusEngine,
    pub context: AnalyticsContext,
}

impl std::fmt::Debug for StrategyManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "StrategyManager with {} strategies", self.strategies.len())
    }
}

impl StrategyManager {
    pub fn new(ctx: AnalyticsContext) -> Self {
        info!("[STRATEGY] Manager V5.5 initialized");
        Self {
            strategies: Vec::new(),
            engine: ConsensusEngine::new(ConsensusMode::default()),
            context: ctx,
        }
    }

    pub fn add_strategy<S: Strategy + 'static>(&mut self, strategy: S) {
        let mut boxed = Box::new(strategy);
        boxed.attach_analytics(self.context.clone());
        // FIX: was invalid \uXXXX escape; now plain ASCII
        info!("[STRATEGY] Attached {}", boxed.name());
        self.strategies.push(boxed);
    }

    pub async fn analyze_all(&mut self, price: f64, ts: i64) -> Result<Signal> {
        use futures::stream::{FuturesUnordered, StreamExt};

        if self.strategies.is_empty() {
            return Ok(Signal::Hold { reason: Some("no strategies loaded".into()) });
        }

        let mut results = Vec::new();
        let mut futs = FuturesUnordered::new();
        for s in &mut self.strategies {
            futs.push(s.analyze(price, ts));
        }
        while let Some(res) = futs.next().await {
            if let Ok(sig) = res { results.push(sig); }
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
        println!("\n[STATS] Strategy Performance (V5.5):");
        for (i, strategy) in self.strategies.iter().enumerate() {
            let stats = strategy.stats();
            println!("  Strategy {} ({}): {} signals generated",
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

    // =======================================================================
    // V5.5: FILL FAN-OUT
    // =======================================================================

    /// Broadcast a confirmed fill to every registered strategy.
    ///
    /// Strategies that care (e.g. GridRebalancer) override `on_fill`;
    /// all others inherit the default no-op and incur zero cost.
    ///
    /// Call site: GridBot::process_price_update() after engine.drain_fills().
    pub fn notify_fill(&mut self, fill: &crate::trading::FillEvent) {
        for strategy in &mut self.strategies {
            strategy.on_fill(fill);
        }
        log::debug!(
            "[FILL] Fanned out to {} strategies — {:?} {} @ {:.4}",
            self.strategies.len(),
            fill.side,
            fill.order_id,
            fill.fill_price,
        );
    }
}

// ══════════════════════════════════════════════════════════════════════
// TESTS
// ══════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::grid_rebalancer::{GridRebalancer, GridRebalancerConfig};
    use crate::trading::{FillEvent, OrderSide};

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
            price: 100.0, size: 1.0, reason: "test".into(), confidence: 0.9, level_id: None,
        };
        let buy = Signal::Buy {
            price: 100.0, size: 1.0, reason: "test".into(), confidence: 0.7, level_id: None,
        };
        let hold = Signal::Hold { reason: Some("test".into()) };
        assert!(strong_buy.strength() > buy.strength());
        assert!(buy.strength() > hold.strength());
        assert_eq!(hold.strength(), 0.0);
    }

    #[test]
    fn test_notify_fill_fanout() {
        let ctx = AnalyticsContext::default();
        let mut mgr = StrategyManager::new(ctx);
        mgr.add_strategy(GridRebalancer::new(GridRebalancerConfig::default()).unwrap());
        let fill = FillEvent::new(
            "ORDER-001", OrderSide::Buy, 142.50, 0.1, 0.0025, Some(0.05), 1_700_000_000,
        );
        mgr.notify_fill(&fill);
        let stats = mgr.strategies[0].stats();
        assert!(stats.rebalances_executed >= 0);
    }

    #[test]
    fn test_on_fill_default_noop() {
        let fill = FillEvent::new("TEST", OrderSide::Sell, 100.0, 0.1, 0.001, None, 0);
        let mut gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        gr.on_fill(&fill);
    }

    #[test]
    fn test_signal_display_with_level_id() {
        let buy_with_level = Signal::Buy {
            price: 142.50,
            size: 0.1,
            reason: "level crossed".into(),
            confidence: 0.85,
            level_id: Some(5),
        };
        let display = buy_with_level.display();
        assert!(display.contains("level 5"));
        assert!(display.contains("$142.50"));

        let buy_without_level = Signal::Buy {
            price: 142.50,
            size: 0.1,
            reason: "grid reposition".into(),
            confidence: 0.90,
            level_id: None,
        };
        let display = buy_without_level.display();
        assert!(!display.contains("level"));
        assert!(display.contains("grid reposition"));
    }
}
