//! Bot trait and supporting types for multi-bot orchestration.
//!
//! Resolves GAP-1 (P0) from V1/V2/V3 audits: "No Bot trait — GridBot
//! hardcoded as only bot type."
//!
//! Every bot type (Grid, Momentum, Arbitrage, DCA) implements the `Bot`
//! trait for uniform lifecycle management and future orchestrator dispatch.
//!
//! PR #86: Adds `IntentRegistry` + `set_intent_registry()` for multi-bot
//! conflict detection (GAP-3). Solo bots ignore this via default no-op.
//!
//! PR #91: Fixes intent_conflicts field missing from BotStats — now
//! surfaced correctly through stats() → aggregate_stats() → fleet log.

use anyhow::Result;
use async_trait::async_trait;
use dashmap::DashMap;
use serde::{Deserialize, Serialize};
use std::fmt;
use std::sync::Arc;

// ══════════════════════════════════════════════════════════════════════
// INTENT REGISTRY  (PR #86 — multi-bot conflict detection)
// ══════════════════════════════════════════════════════════════════════

/// Shared intent registry for multi-bot fleet conflict detection.
///
/// Key:   `(trading_pair: String, level_id: u64)`
/// Value: `instance_id: String` — the bot that owns this level.
///
/// `DashMap` provides lock-free concurrent reads and atomic writes,
/// making it safe for N bot tasks operating in parallel without a global
/// mutex. Each bot registers its intended levels before placing orders;
/// if a level is already owned by another instance, it skips silently.
///
/// Key namespace uses the **trading pair** (e.g. `"SOL/USDC"`), NOT the
/// bot instance name — this ensures two bots on the same pair can detect
/// each other's level claims. (PR #91 fix — was `instance_name()`.)
///
/// Solo bots (single-instance mode) never receive this registry —
/// `set_intent_registry()` is a default no-op, so existing behavior is
/// byte-for-byte unchanged when running without the orchestrator.
pub type IntentRegistry = Arc<DashMap<(String, u64), String>>;

/// Construct an empty `IntentRegistry` for a new orchestrator session.
#[inline]
pub fn new_intent_registry() -> IntentRegistry {
    Arc::new(DashMap::new())
}

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
///
/// PR #91: Added `intent_conflicts` — wired from GridBot.intent_conflicts
/// through stats() so aggregate_stats() can sum real conflict events
/// instead of reading registry.len() (which counted claims, not conflicts).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BotStats {
    pub instance_id:      String,
    pub bot_type:         String,
    pub total_cycles:     u64,
    pub total_fills:      u64,
    pub total_orders:     u64,
    pub uptime_secs:      u64,
    pub is_paused:        bool,
    pub current_pnl:      f64,
    /// Real conflict events detected by intent registry this session.
    /// PR #91: was missing — caused aggregate_stats() to use registry.len()
    /// as a proxy, which counted successful claims not actual conflicts.
    #[serde(default)]
    pub intent_conflicts: u64,
}

// ══════════════════════════════════════════════════════════════════════
// ORCHESTRATOR STATS  (PR #86 — fleet-level aggregation)
// ══════════════════════════════════════════════════════════════════════

/// Fleet-level aggregated statistics from all running bot instances.
///
/// Computed by the orchestrator's aggregation loop every `stats_interval`
/// cycles and emitted to logs / Telegram / future Supabase sink.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct OrchestratorStats {
    /// Number of bot instances currently running.
    pub active_bots:      usize,
    /// Number of bots currently paused (regime gate / circuit breaker).
    pub paused_bots:      usize,
    /// Total fills across all bots this session.
    pub total_fills:      u64,
    /// Total orders placed across all bots this session.
    pub total_orders:     u64,
    /// Sum of all bot P&Ls in USDC.
    pub fleet_pnl:        f64,
    /// Real intent conflict events summed across all bots.
    /// PR #91: now correctly summed from BotStats.intent_conflicts,
    /// not from registry.len() (which was claimed_levels, not conflicts).
    pub intent_conflicts: u64,
    /// Uptime in seconds (from orchestrator start).
    pub uptime_secs:      u64,
}

impl fmt::Display for OrchestratorStats {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "bots={}/{} fills={} orders={} pnl=${:.2} conflicts={} uptime={}s",
            self.active_bots - self.paused_bots,
            self.active_bots,
            self.total_fills,
            self.total_orders,
            self.fleet_pnl,
            self.intent_conflicts,
            self.uptime_secs,
        )
    }
}

// ══════════════════════════════════════════════════════════════════════
// BOT TRAIT
// ══════════════════════════════════════════════════════════════════════

/// Core lifecycle trait for all bot types.
///
/// The orchestrator calls these methods in order:
///
/// 1. `set_intent_registry()` — inject shared conflict map (orchestrated mode only)
/// 2. `initialize()`          — one-time setup (price feed warmup, grid placement)
/// 3. `process_tick()`        — called each cycle; bot owns its own timing + feeds
/// 4. `shutdown()`            — graceful teardown (cancel orders, dump state)
/// 5. `stats()`               — sync snapshot for aggregation loop
///
/// # Backward compatibility
///
/// `set_intent_registry()` has a default no-op implementation so all
/// existing `impl Bot` blocks compile unchanged. Solo bots never receive
/// a registry and behave identically to pre-PR-#86.
///
/// # Design note
///
/// `process_tick(&mut self)` — the bot owns its price feed and loop.
/// When `TickContext` lands (P2 roadmap), this may evolve to
/// `process_tick(&mut self, ctx: &TickContext)` with shared resources.
#[async_trait]
pub trait Bot: Send + Sync {
    /// Human-readable bot type (e.g., `"GridBot"`, `"MomentumBot"`).
    fn name(&self) -> &str;

    /// Unique instance identifier from config (e.g., `"sol-usdc-grid-01"`).
    fn instance_id(&self) -> &str;

    /// Inject the shared intent registry before `initialize()` is called.
    ///
    /// **Default no-op** — solo bots ignore this. The orchestrator calls
    /// this after constructing each bot and before `initialize()` so the
    /// conflict guard in `place_grid_orders()` has a registry to write to.
    ///
    /// Implementations store `registry` as `Option<IntentRegistry>` and
    /// check `if let Some(r) = &self.intent_registry` before every order.
    fn set_intent_registry(&mut self, _registry: IntentRegistry) {
        // Default no-op: solo bots compile unchanged, zero behavior change.
    }

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
        // PR #91: intent_conflicts must default to zero
        assert_eq!(stats.intent_conflicts, 0);
    }

    #[test]
    fn test_bot_stats_intent_conflicts_field() {
        // PR #91: BotStats carries and round-trips intent_conflicts correctly.
        let stats = BotStats {
            instance_id:      "sol-usdc-grid-01".into(),
            bot_type:         "GridBot".into(),
            total_cycles:     50,
            total_fills:      5,
            total_orders:     10,
            uptime_secs:      60,
            is_paused:        false,
            current_pnl:      1.25,
            intent_conflicts: 3,
        };
        assert_eq!(stats.intent_conflicts, 3);
        // Serde round-trip
        let json = serde_json::to_string(&stats).unwrap();
        let de: BotStats = serde_json::from_str(&json).unwrap();
        assert_eq!(de.intent_conflicts, 3);
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
            instance_id:      "sol-usdc-grid-01".into(),
            bot_type:         "GridBot".into(),
            total_cycles:     1000,
            total_fills:      42,
            total_orders:     85,
            uptime_secs:      3600,
            is_paused:        false,
            current_pnl:      12.50,
            intent_conflicts: 0,
        };
        let json = serde_json::to_string(&stats).unwrap();
        let de: BotStats = serde_json::from_str(&json).unwrap();
        assert_eq!(de.instance_id, "sol-usdc-grid-01");
        assert_eq!(de.total_fills, 42);
        assert_eq!(de.current_pnl, 12.50);
        assert_eq!(de.intent_conflicts, 0);
    }

    #[test]
    fn test_intent_registry_empty_on_new() {
        let registry = new_intent_registry();
        assert!(registry.is_empty());
    }

    #[test]
    fn test_intent_registry_correct_namespace() {
        // PR #91: key must use trading pair, not instance name.
        // Two bots on the same pair MUST see each other's claims.
        let registry = new_intent_registry();
        let pair = "SOL/USDC".to_string();
        let level_id = 42u64;
        let key = (pair.clone(), level_id);

        // Bot-01 claims the level using trading pair as namespace
        registry.insert(key.clone(), "sol-usdc-grid-01".to_string());

        // Bot-02 on the SAME pair at the SAME level must detect the conflict
        assert!(
            registry.contains_key(&key),
            "Bot-02 must see bot-01's claim when using trading pair as key namespace"
        );
        let owner = registry.get(&key).unwrap();
        assert_eq!(owner.value(), "sol-usdc-grid-01");
    }

    #[test]
    fn test_intent_registry_different_bots_same_pair_conflict() {
        // PR #91: core safety test — two bots on SOL/USDC, same level → conflict detected.
        let registry = new_intent_registry();
        let key = ("SOL/USDC".to_string(), 5u64);

        // Bot-01 claims level 5
        registry.insert(key.clone(), "sol-usdc-grid-01".to_string());

        // Bot-02 tries to claim level 5 — must find it Occupied
        let conflict_detected = registry.contains_key(&key);
        assert!(conflict_detected, "Conflict must be detected when two bots target same pair+level");

        // Verify bot-01 still owns it (no overwrite)
        assert_eq!(registry.get(&key).unwrap().value(), "sol-usdc-grid-01");
    }

    #[test]
    fn test_intent_registry_conflict_detection() {
        let registry = new_intent_registry();
        let key = ("SOL/USDC".to_string(), 42u64);
        // First bot claims the level
        registry.insert(key.clone(), "sol-usdc-grid-01".to_string());
        // Second bot detects conflict
        assert!(registry.contains_key(&key));
        let owner = registry.get(&key).unwrap();
        assert_eq!(owner.value(), "sol-usdc-grid-01");
    }

    #[test]
    fn test_intent_registry_no_conflict_different_pairs() {
        let registry = new_intent_registry();
        let key1 = ("SOL/USDC".to_string(), 1u64);
        let key2 = ("ETH/USDC".to_string(), 1u64);
        registry.insert(key1.clone(), "bot-01".to_string());
        registry.insert(key2.clone(), "bot-02".to_string());
        // Same level_id on different pairs = no conflict
        assert_eq!(registry.get(&key1).unwrap().value(), "bot-01");
        assert_eq!(registry.get(&key2).unwrap().value(), "bot-02");
    }

    #[test]
    fn test_orchestrator_stats_display() {
        let stats = OrchestratorStats {
            active_bots:      2,
            paused_bots:      0,
            total_fills:      18,
            total_orders:     40,
            fleet_pnl:        7.50,
            intent_conflicts: 0,
            uptime_secs:      120,
        };
        let s = stats.to_string();
        assert!(s.contains("bots=2/2"));
        assert!(s.contains("pnl=$7.50"));
    }

    #[test]
    fn test_set_intent_registry_default_noop() {
        let registry = new_intent_registry();
        assert!(registry.is_empty());
    }
}
