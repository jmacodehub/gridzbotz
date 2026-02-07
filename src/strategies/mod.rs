// 🎯 PROJECT FLASH V5.3 - STRATEGY ENGINE (Monitor-Enhanced Edition)
// ═══════════════════════════════════════════════════════════════════════════
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
// ═══════════════════════════════════════════════════════════════════════════

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

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY TRAIT - ASYNC AND CONTEXT-AWARE
// ═══════════════════════════════════════════════════════════════════════════
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

// ═══════════════════════════════════════════════════════════════════════════
// SIGNAL STRUCTURE - UNIFIED CROSS-MODULE STANDARD
// ═══════════════════════════════════════════════════════════════════════════
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
    },
    Hold {
        reason: Option<String>,
    },
    Sell {
        price: f64,
        size: f64,
        reason: String,
        confidence: f64,
    },
    StrongSell {
        price: f64,
        size: f64,
        reason: String,
        confidence: f64,
    },
}

impl Signal {
    pub fn to_order_side(&self) -> Optionrate::trading::OrderSide> {
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
            Signal::StrongBuy {
                price,
                reason,
                confidence,
                ..
            } => format!(
                "🟢 STRONG BUY @ ${:.4} | {} | {:.0}% conf",
                price,
                reason,
                confidence * 100.0
            ),
            Signal::Buy {
                price,
                reason,
                confidence,
                ..
            } => format!(
                "🟩 BUY @ ${:.4} | {} | {:.0}%",
                price,
                reason,
                confidence * 100.0
            ),
            Signal::Hold { reason } => format!(
                "⏸️ HOLD | {}",
                reason.clone().unwrap_or_else(|| "Neutral".into())
            ),
            Signal::Sell {
                price,
                reason,
                confidence,
                ..
            } => format!(
                "🟥 SELL @ ${:.4} | {} | {:.0}%",
                price,
                reason,
                confidence * 100.0
            ),
            Signal::StrongSell {
                price,
                reason,
                confidence,
                ..
            } => format!(
                "🔴 STRONG SELL @ ${:.4} | {} | {:.0}%",
                price,
                reason,
                confidence * 100.0
            ),
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.display())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY STATS - LIGHTWEIGHT PERFORMANCE METRICS
// ═══════════════════════════════════════════════════════════════════════════
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct StrategyStats {
    pub signals_generated: u64,
    pub active_trades: u64,
    pub total_pnl: f64,
    pub win_rate: f64,
    pub sharpe: f64,
    pub rebalances_executed: u64,
}

// ═══════════════════════════════════════════════════════════════════════════
// STRATEGY MANAGER - ASYNC CONSENSUS ORCHESTRATOR
// ═══════════════════════════════════════════════════════════════════════════
pub struct StrategyManager {
    pub strategies: Vec<Box<dyn Strategy>>,
    pub engine: ConsensusEngine,
    pub context: AnalyticsContext,
}

impl std::fmt::Debug for StrategyManager {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "StrategyManager with {} strategies",
            self.strategies.len()
        )
    }
}

impl StrategyManager {
    pub fn new(ctx: AnalyticsContext) -> Self {
        info!("🧠 Strategy Manager V5.3 initialized");
        Self {
            strategies: Vec::new(),
            engine: ConsensusEngine::new(ConsensusMode::default()),
            context: ctx,
        }
    }

    pub fn add_strategy<S: Strategy + 'static>(&mut self, strategy: S) {
        let mut boxed = Box::new(strategy);
        boxed.attach_analytics(self.context.clone());
        info!("📈 Attached {}", boxed.name());
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
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE - CONSENSUS AND SIGNAL PIPELINE
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::grid_rebalancer::{GridRebalancer, GridRebalancerConfig};

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn test_manager_consensus_integration() {
        let ctx = AnalyticsContext::default();
        let mut mgr = StrategyManager::new(ctx);
        mgr.engine.mode = ConsensusMode::MajorityVote;
        let regime_cfg = crate::config::RegimeGateConfig::default();
        mgr.add_strategy(GridRebalancer::new(GridRebalancerConfig::default(), &regime_cfg).unwrap());

        let sig = mgr.analyze_all(100.0, 1).await.unwrap();
        assert!(matches!(
            sig,
            Signal::Buy { .. } | Signal::Hold { .. } | Signal::Sell { .. }
        ));
    }
}
