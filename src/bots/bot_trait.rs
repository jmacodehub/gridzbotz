//! Bot trait and supporting types for multi-bot orchestration.
//!
//! Resolves GAP-1 (P0) from V1/V2/V3 audits: "No Bot trait — GridBot
//! hardcoded as only bot type."
//!
//! Every bot type (Grid, Momentum, Arbitrage, DCA) implements the `Bot`
//! trait for uniform lifecycle management and future orchestrator dispatch.

use anyhow::Result;
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::fmt;

// ══════════════════════════════════════════════════════════════════════
// TICK RESULT
// ══════════════════════════════════════════════════════════════════════

/// Outcome of a single `process_tick()` cycle.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TickResult {
    /// Number of fills executed this tick.
    pub fills: u64,
    /// Number of orders placed this tick.
    pub orders_placed: u64,
    /// `true` while the bot should keep running.
    pub active: bool,
    /// Set when the bot is paused (circuit breaker, regime gate, etc.).
    pub pause_reason: Option<String>,
}

impl TickResult {
    /// Normal active tick with fill/order counts.
    pub fn active(fills: u64, orders_placed: u64) -> Self {
        Self {
            fills,
            orders_placed,
            active: true,
            pause_reason: None,
        }
    }

    /// Bot is alive but temporarily paused.
    pub fn paused(reason: impl Into<String>) -> Self {
        Self {
            fills: 0,
            orders_placed: 0,
            active: true,
            pause_reason: Some(reason.into()),
        }
    }

    /// Bot requests permanent shutdown.
    pub fn shutdown() -> Self {
        Self {
            fills: 0,
            orders_placed: 0,
            active: false,
            pause_reason: None,
        }
    }
}

impl fmt::Display for TickResult {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if !self.active {
            write!(f, "SHUTDOWN")
        } else if let Some(ref reason) = self.pause_reason {
            write!(f, "PAUSED: {reason}")
        } else {
            write!(f, "fills={} orders={}", self.fills, self.orders_placed)
        }
    }
}

// ══════════════════════════════════════════════════════════════════════
// BOT STATS
// ══════════════════════════════════════════════════════════════════════

/// Aggregated bot-level statistics for observability and dashboards.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotStats {
    pub instance_id: String,
    pub bot_type: String,
    pub total_cycles: u64,
    pub total_fills: u64,
    pub total_orders: u64,
    pub uptime_secs: u64,
    pub is_paused: bool,
    pub current_pnl: f64,
}

// ══════════════════════════════════════════════════════════════════════
// BOT TRAIT
// ══════════════════════════════════════════════════════════════════════

/// Core lifecycle trait for all bot types.
///
/// The orchestrator (future GAP-3) calls these methods in order:
///
/// 1. `initialize()` — one-time setup (price feed warmup, grid placement)
/// 2. `process_tick()` — called each cycle; bot owns its own timing + feeds
/// 3. `shutdown()`    — graceful teardown (cancel orders, dump state)
///
/// # Design note
///
/// `process_tick(&mut self)` — the bot owns its price feed and loop.
/// When the multi-bot orchestrator (GAP-3) lands, this may evolve to
/// `process_tick(&mut self, ctx: &TickContext)` with shared resources.
#[async_trait]
pub trait Bot: Send + Sync {
    /// Human-readable bot type (e.g., `"GridBot"`, `"MomentumBot"`).
    fn name(&self) -> &str;

    /// Unique instance identifier from config (e.g., `"sol-usdc-grid-01"`).
    fn instance_id(&self) -> &str;

    /// One-time initialization before the trading loop begins.
    async fn initialize(&mut self) -> Result<()>;

    /// Execute one trading cycle.
    ///
    /// The bot fetches price, runs strategy analysis, places/cancels orders,
    /// and returns a [`TickResult`] summarising what happened.
    async fn process_tick(&mut self) -> Result<TickResult>;

    /// Graceful shutdown: cancel open orders, flush logs, dump state.
    async fn shutdown(&mut self) -> Result<()>;

    /// Current aggregated statistics.
    fn stats(&self) -> BotStats;
}

// ══════════════════════════════════════════════════════════════════════
// TESTS
// ══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tick_result_active() {
        let r = TickResult::active(3, 5);
        assert!(r.active);
        assert_eq!(r.fills, 3);
        assert_eq!(r.orders_placed, 5);
        assert!(r.pause_reason.is_none());
        assert_eq!(r.to_string(), "fills=3 orders=5");
    }

    #[test]
    fn test_tick_result_paused() {
        let r = TickResult::paused("circuit breaker tripped");
        assert!(r.active);
        assert_eq!(r.fills, 0);
        assert!(r.pause_reason.is_some());
        assert!(r.to_string().contains("PAUSED"));
        assert!(r.to_string().contains("circuit breaker"));
    }

    #[test]
    fn test_tick_result_shutdown() {
        let r = TickResult::shutdown();
        assert!(!r.active);
        assert_eq!(r.to_string(), "SHUTDOWN");
    }

    #[test]
    fn test_bot_stats_default() {
        let stats = BotStats::default();
        assert_eq!(stats.total_cycles, 0);
        assert_eq!(stats.total_fills, 0);
        assert!(!stats.is_paused);
        assert_eq!(stats.current_pnl, 0.0);
    }

    #[test]
    fn test_tick_result_serde_roundtrip() {
        let original = TickResult::active(7, 12);
        let json = serde_json::to_string(&original).unwrap();
        let deserialized: TickResult = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.fills, 7);
        assert_eq!(deserialized.orders_placed, 12);
        assert!(deserialized.active);
    }

    #[test]
    fn test_bot_stats_serde_roundtrip() {
        let stats = BotStats {
            instance_id: "sol-usdc-grid-01".into(),
            bot_type: "GridBot".into(),
            total_cycles: 1000,
            total_fills: 42,
            total_orders: 85,
            uptime_secs: 3600,
            is_paused: false,
            current_pnl: 12.50,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let de: BotStats = serde_json::from_str(&json).unwrap();
        assert_eq!(de.instance_id, "sol-usdc-grid-01");
        assert_eq!(de.total_fills, 42);
        assert_eq!(de.current_pnl, 12.50);
    }
}
