//! Strategy registry for config-driven strategy construction.
//!
//! Resolves GAP-2 from V1/V2/V3 audits: "Hardcoded strategy registration
//! via if/else blocks in gridbot.rs."
//!
//! Provides:
//! - [`StrategyEntry`]: a strategy bundled with its weight, enabled flag,
//!   and `wma_voter` flag (PR #105)
//! - [`StrategyRegistryBuilder`]: accumulates entries → builds `StrategyManager`
//!
//! PR #98: `build()` calls `wma_engine.register_strategy()` for every
//! enabled strategy with `wma_voter=true` so WMA has a performance-tracking
//! slot from the first tick.
//!
//! PR #99 Commit 3a: `build_with_confidence(ctx, threshold)` added —
//! config-driven path that passes `wma_confidence_threshold` from TOML
//! all the way into `WMAConsensusEngine::with_min_confidence()`.
//!
//! PR #105: `add_execution_only()` added. Strategies registered via this
//! path participate in `on_fill()` callbacks and price initialization but
//! are NOT registered with `wma_engine`. Use for modules that place orders
//! via their own logic (e.g. `GridRebalancer`) rather than WMA voting.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // WMA voter (signal strategy — votes in consensus):
//! let (manager, weights) = StrategyRegistryBuilder::new()
//!     .add(momentum_strategy, 0.30)
//!     .add_if(rsi_enabled, rsi_strategy, 0.30)
//!     .build_with_confidence(analytics_context, config.strategies.wma_confidence_threshold);
//!
//! // Execution-only (grid rebalancer — places orders directly, not via WMA):
//! let (manager, weights) = StrategyRegistryBuilder::new()
//!     .add_execution_only(grid_rebalancer, 0.40)
//!     .add_if(momentum_enabled, momentum_strategy, 0.20)
//!     .build_with_confidence(analytics_context, config.strategies.wma_confidence_threshold);
//! ```

use crate::strategies::{AnalyticsContext, Strategy, StrategyManager};
use log::info;

// ══════════════════════════════════════════════════════════════════════
// STRATEGY ENTRY
// ══════════════════════════════════════════════════════════════════════

/// A strategy bundled with its configured weight, enabled state, and
/// WMA voter flag.
///
/// Weights seed WMA initial importance. A weight of `0.0` marks
/// non-signal strategies (e.g., SmartFeeFilter).
///
/// `wma_voter = true` (default): strategy is registered with `wma_engine`
/// and participates in consensus voting.
///
/// `wma_voter = false`: strategy receives `on_fill()` callbacks and
/// `attach_analytics()` but is NOT registered with `wma_engine`. Use for
/// execution modules like `GridRebalancer` whose `analyze()` always returns
/// `Signal::Hold` — registering them in WMA creates a permanent 0.0-confidence
/// voter that deadlocks the consensus gate. (PR #105)
pub struct StrategyEntry {
    pub strategy: Box<dyn Strategy>,
    pub weight: f64,
    pub enabled: bool,
    /// When `false`, this entry is skipped during `wma_engine.register_strategy()`.
    /// Default: `true`. Set to `false` via `add_execution_only()`.
    pub wma_voter: bool,
}

impl StrategyEntry {
    /// Create a new enabled WMA-voter entry with the given weight.
    pub fn new(strategy: impl Strategy + 'static, weight: f64) -> Self {
        Self {
            strategy: Box::new(strategy),
            weight,
            enabled: true,
            wma_voter: true,  // default: participates in WMA consensus
        }
    }

    /// Strategy name (delegates to inner trait).
    pub fn name(&self) -> &str {
        self.strategy.name()
    }
}

impl std::fmt::Debug for StrategyEntry {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "StrategyEntry {{ name: {:?}, weight: {:.2}, enabled: {}, wma_voter: {} }}",
            self.name(),
            self.weight,
            self.enabled,
            self.wma_voter,
        )
    }
}

// ══════════════════════════════════════════════════════════════════════
// REGISTRY BUILDER
// ══════════════════════════════════════════════════════════════════════

/// Accumulates strategy entries and produces a configured `StrategyManager`.
///
/// Replaces the 200-line if/else block in `gridbot.rs` with a clean,
/// composable builder pattern.
///
/// ## Construction paths
///
/// | Method | Manager constructor | WMA conf gate |
/// |---|---|---|
/// | `build(ctx)` | `StrategyManager::new()` | 0.65 (historic default) |
/// | `build_with_confidence(ctx, t)` | `StrategyManager::new_with_confidence()` | TOML value |
///
/// ## Registration paths
///
/// | Method | WMA voter | on_fill callbacks |
/// |---|---|---|
/// | `add()` | ✅ yes | ✅ yes |
/// | `add_if()` | ✅ yes (if enabled) | ✅ yes |
/// | `add_execution_only()` | ❌ no | ✅ yes |
/// | `add_if_execution_only()` | ❌ no (if enabled) | ✅ yes |
///
/// PR #98: `build()` and `build_with_confidence()` register every enabled
/// strategy with `wma_voter=true` with `wma_engine` so the WMA performance
/// tracker has a slot ready before the first tick.
///
/// PR #105: `add_execution_only()` skips `wma_engine` registration.
/// Use for `GridRebalancer` and any module whose `analyze()` returns
/// `Signal::Hold` — avoids permanent 0.0-confidence WMA deadlock.
pub struct StrategyRegistryBuilder {
    entries: Vec<StrategyEntry>,
}

impl StrategyRegistryBuilder {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a strategy as a WMA voter (unconditionally).
    ///
    /// Use for signal strategies: Momentum, RSI, MeanReversion, etc.
    /// The strategy will receive a WMA performance slot and participate
    /// in consensus voting via `analyze()` → confidence score.
    pub fn add(mut self, strategy: impl Strategy + 'static, weight: f64) -> Self {
        let entry = StrategyEntry::new(strategy, weight);
        info!(
            "[REGISTRY] Registered (WMA voter): {} (weight={:.2})",
            entry.name(),
            weight,
        );
        self.entries.push(entry);
        self
    }

    /// Register a strategy as a WMA voter only if `enabled` is `true`.
    pub fn add_if(
        self,
        enabled: bool,
        strategy: impl Strategy + 'static,
        weight: f64,
    ) -> Self {
        if enabled {
            self.add(strategy, weight)
        } else {
            self
        }
    }

    /// Register an execution-only strategy: participates in `on_fill()`
    /// callbacks and `attach_analytics()` but is **NOT** registered with
    /// `wma_engine`.
    ///
    /// ## When to use
    /// Use for modules that place orders via their own logic (e.g.
    /// `GridRebalancer` via `should_place_order()`) rather than emitting
    /// a consensus signal through `analyze()`.
    ///
    /// ## Why this matters (PR #105)
    /// `GridRebalancer::analyze()` always returns `Signal::Hold`
    /// (confidence=0.0). If registered as a WMA voter it permanently
    /// occupies a slot emitting 0.0, always below `wma_conf_gate`.
    /// This creates a deadlock: no fills → no weight update → always 0.0.
    /// Using `add_execution_only()` eliminates the slot entirely.
    pub fn add_execution_only(
        mut self,
        strategy: impl Strategy + 'static,
        weight: f64,
    ) -> Self {
        let mut entry = StrategyEntry::new(strategy, weight);
        entry.wma_voter = false;  // ← PR #105: skip WMA registration
        info!(
            "[REGISTRY] Registered (execution-only, WMA excluded): {} (weight={:.2})",
            entry.name(),
            weight,
        );
        self.entries.push(entry);
        self
    }

    /// Register an execution-only strategy only if `enabled` is `true`.
    pub fn add_if_execution_only(
        self,
        enabled: bool,
        strategy: impl Strategy + 'static,
        weight: f64,
    ) -> Self {
        if enabled {
            self.add_execution_only(strategy, weight)
        } else {
            self
        }
    }

    /// Number of registered (enabled) strategies.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Returns `true` if no strategies are registered.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Consume the builder and produce a configured `StrategyManager`.
    ///
    /// Uses `StrategyManager::new()` — historic 0.65 confidence gate.
    /// Suitable for tests and any call site that doesn't own a `Config`.
    ///
    /// For the config-driven path, use `build_with_confidence()` instead.
    ///
    /// Returns `(manager, weights)` — weights are parallel to the strategies
    /// vector and seed the WMA engine's initial importance per strategy.
    pub fn build(self, ctx: AnalyticsContext) -> (StrategyManager, Vec<f64>) {
        self.build_inner(ctx, None)
    }

    /// Consume the builder and produce a configured `StrategyManager` with a
    /// config-driven WMA confidence gate.
    ///
    /// PR #99 Commit 3a: this is the canonical path for `GridBot::new()`.
    /// Pass `config.strategies.wma_confidence_threshold` as `wma_confidence`.
    ///
    /// Returns `(manager, weights)` — identical contract to `build()`.
    pub fn build_with_confidence(
        self,
        ctx: AnalyticsContext,
        wma_confidence: f64,
    ) -> (StrategyManager, Vec<f64>) {
        self.build_inner(ctx, Some(wma_confidence))
    }

    // ─────────────────────────────────────────────────────────────────────
    // Private implementation shared by both public builders.
    // `wma_confidence = None`    → StrategyManager::new()          (0.65)
    // `wma_confidence = Some(t)` → StrategyManager::new_with_confidence(t)
    //
    // PR #105: only entries with wma_voter=true are passed to
    // wma_engine.register_strategy(). Execution-only entries skip that call
    // but are still pushed into manager.strategies for on_fill() dispatch.
    // ─────────────────────────────────────────────────────────────────────
    fn build_inner(
        self,
        ctx: AnalyticsContext,
        wma_confidence: Option<f64>,
    ) -> (StrategyManager, Vec<f64>) {
        let mut manager = match wma_confidence {
            Some(threshold) => StrategyManager::new_with_confidence(ctx, threshold),
            None            => StrategyManager::new(ctx),
        };
        let mut weights = Vec::with_capacity(self.entries.len());
        let mut wma_voter_count: usize = 0;

        for mut entry in self.entries {
            if entry.enabled {
                let name = entry.strategy.name().to_string();
                weights.push(entry.weight);
                entry.strategy.attach_analytics(manager.context.clone());

                if entry.wma_voter {
                    // PR #98: register slot so WMA tracks this voter from tick 1.
                    manager.wma_engine.register_strategy(name.clone());
                    wma_voter_count += 1;
                    info!("[REGISTRY] Attached: {} (WMA voter, weight={:.2})", name, entry.weight);
                } else {
                    // PR #105: execution-only — on_fill() works, WMA slot skipped.
                    info!("[REGISTRY] Attached: {} (execution-only, WMA excluded, weight={:.2})", name, entry.weight);
                }

                manager.strategies.push(entry.strategy);
            }
        }

        info!(
            "[REGISTRY] Built StrategyManager: {} strategies ({} WMA voters, conf_gate={:.2})",
            manager.strategies.len(),
            wma_voter_count,
            manager.wma_engine.min_confidence(),
        );

        (manager, weights)
    }
}

impl Default for StrategyRegistryBuilder {
    fn default() -> Self {
        Self::new()
    }
}

// ══════════════════════════════════════════════════════════════════════
// TESTS
// ══════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;
    use crate::strategies::grid_rebalancer::{GridRebalancer, GridRebalancerConfig};

    #[test]
    fn test_strategy_entry_creation() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let entry = StrategyEntry::new(gr, 0.40);
        assert_eq!(entry.weight, 0.40);
        assert!(entry.enabled);
        assert!(!entry.name().is_empty());
    }

    #[test]
    fn test_strategy_entry_debug() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let entry = StrategyEntry::new(gr, 0.40);
        let debug = format!("{:?}", entry);
        assert!(debug.contains("weight: 0.40"));
        assert!(debug.contains("enabled: true"));
    }

    #[test]
    fn test_registry_builder_add() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let builder = StrategyRegistryBuilder::new().add(gr, 0.40);
        assert_eq!(builder.len(), 1);
    }

    #[test]
    fn test_registry_builder_add_if_enabled() {
        let gr1 = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let gr2 = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let builder = StrategyRegistryBuilder::new()
            .add_if(true, gr1, 0.40)
            .add_if(false, gr2, 0.20);
        assert_eq!(builder.len(), 1);
    }

    #[test]
    fn test_registry_builder_empty() {
        let builder = StrategyRegistryBuilder::new();
        assert!(builder.is_empty());
        assert_eq!(builder.len(), 0);
    }

    #[test]
    fn test_registry_builder_build() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let builder = StrategyRegistryBuilder::new().add(gr, 0.40);
        let ctx = AnalyticsContext::default();
        let (manager, weights) = builder.build(ctx);
        assert_eq!(manager.strategies.len(), 1);
        assert_eq!(weights.len(), 1);
        assert_eq!(weights[0], 0.40);
    }

    #[test]
    fn test_registry_builder_preserves_weight_order() {
        let gr1 = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let gr2 = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let builder = StrategyRegistryBuilder::new()
            .add(gr1, 0.40)
            .add(gr2, 0.25);
        let ctx = AnalyticsContext::default();
        let (manager, weights) = builder.build(ctx);
        assert_eq!(manager.strategies.len(), 2);
        assert_eq!(weights, vec![0.40, 0.25]);
    }

    // ────────────────────────────────────────────────────────────────────────
    // PR #98 Commit 1: WMA registration via builder
    // ────────────────────────────────────────────────────────────────────────

    /// build() must register every enabled WMA-voter strategy with wma_engine.
    /// Updated PR #105: uses .add() for voter path, not .add_execution_only().
    #[test]
    fn test_registry_build_registers_wma_slots() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let builder = StrategyRegistryBuilder::new().add(gr, 0.40);
        let ctx = AnalyticsContext::default();
        let (manager, _) = builder.build(ctx);
        let perf = manager.wma_engine.get_performance("GridRebalancer");
        assert!(perf.is_some(), "WMA must have a slot for a strategy added via .add()");
    }

    /// Disabled strategies must NOT be registered with wma_engine.
    #[test]
    fn test_registry_disabled_strategy_not_in_wma() {
        let gr1 = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let gr2 = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let builder = StrategyRegistryBuilder::new()
            .add(gr1, 0.40)
            .add_if(false, gr2, 0.20);
        let ctx = AnalyticsContext::default();
        let (manager, _) = builder.build(ctx);
        assert_eq!(manager.strategies.len(), 1);
        let perf = manager.wma_engine.get_performance("GridRebalancer");
        assert!(perf.is_some());
    }

    // ────────────────────────────────────────────────────────────────────────
    // PR #99 Commit 3a: build_with_confidence() tests
    // ────────────────────────────────────────────────────────────────────────

    /// build_with_confidence() must wire the threshold into the WMA engine.
    #[test]
    fn test_build_with_confidence_wires_threshold() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let ctx = AnalyticsContext::default();
        let (manager, weights) = StrategyRegistryBuilder::new()
            .add(gr, 0.40)
            .build_with_confidence(ctx, 0.75);
        assert_eq!(weights, vec![0.40]);
        assert_eq!(manager.strategies.len(), 1);
        assert!(
            (manager.wma_engine.min_confidence() - 0.75).abs() < 1e-9,
            "conf_gate must be 0.75, got {}", manager.wma_engine.min_confidence()
        );
        assert!(manager.wma_engine.get_performance("GridRebalancer").is_some());
    }

    /// build() (legacy path) must preserve the historic 0.65 gate.
    #[test]
    fn test_build_preserves_default_gate_via_plain_build() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let ctx = AnalyticsContext::default();
        let (manager, _) = StrategyRegistryBuilder::new()
            .add(gr, 0.40)
            .build(ctx);
        assert!(
            (manager.wma_engine.min_confidence() - 0.65).abs() < 1e-9,
            "plain build() must keep 0.65 gate, got {}", manager.wma_engine.min_confidence()
        );
    }

    // ────────────────────────────────────────────────────────────────────────
    // PR #105: add_execution_only() — WMA deadlock fix
    // ────────────────────────────────────────────────────────────────────────

    /// add_execution_only() must NOT create a WMA voter slot.
    /// This is the core regression catch for the GridRebalancer deadlock.
    #[test]
    fn test_execution_only_not_registered_in_wma() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let ctx = AnalyticsContext::default();
        let (manager, weights) = StrategyRegistryBuilder::new()
            .add_execution_only(gr, 0.40)
            .build(ctx);
        assert_eq!(manager.strategies.len(), 1, "strategy must be in manager");
        assert_eq!(weights, vec![0.40]);
        assert!(
            manager.wma_engine.get_performance("GridRebalancer").is_none(),
            "execution-only strategy must NOT have a WMA voter slot"
        );
    }

    /// Strategies added via plain .add() must still register in WMA.
    /// Ensures add_execution_only() didn't break the default path.
    #[test]
    fn test_regular_add_still_registers_in_wma() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let ctx = AnalyticsContext::default();
        let (manager, _) = StrategyRegistryBuilder::new()
            .add(gr, 0.40)
            .build(ctx);
        assert!(
            manager.wma_engine.get_performance("GridRebalancer").is_some(),
            ".add() path must still register a WMA slot"
        );
    }

    /// add_if_execution_only() with enabled=true must register the strategy
    /// as execution-only (in manager, NOT in WMA).
    #[test]
    fn test_add_if_execution_only_enabled() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let ctx = AnalyticsContext::default();
        let (manager, weights) = StrategyRegistryBuilder::new()
            .add_if_execution_only(true, gr, 0.40)
            .build(ctx);
        assert_eq!(manager.strategies.len(), 1);
        assert_eq!(weights, vec![0.40]);
        assert!(
            manager.wma_engine.get_performance("GridRebalancer").is_none(),
            "add_if_execution_only(true) must produce no WMA slot"
        );
    }

    /// add_if_execution_only() with enabled=false must not register anything.
    #[test]
    fn test_add_if_execution_only_disabled() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let ctx = AnalyticsContext::default();
        let (manager, weights) = StrategyRegistryBuilder::new()
            .add_if_execution_only(false, gr, 0.40)
            .build(ctx);
        assert_eq!(manager.strategies.len(), 0);
        assert!(weights.is_empty());
    }

    /// StrategyEntry::new() must default wma_voter to true.
    #[test]
    fn test_strategy_entry_wma_voter_default_true() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let entry = StrategyEntry::new(gr, 0.40);
        assert!(entry.wma_voter, "new entries must default wma_voter=true");
    }

    /// StrategyEntry Debug output must include wma_voter field.
    #[test]
    fn test_strategy_entry_debug_includes_wma_voter() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let entry = StrategyEntry::new(gr, 0.40);
        let debug = format!("{:?}", entry);
        assert!(debug.contains("wma_voter: true"), "debug must show wma_voter field");
    }
}
