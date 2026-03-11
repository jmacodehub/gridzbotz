//! Strategy registry for config-driven strategy construction.
//!
//! Resolves GAP-2 from V1/V2/V3 audits: "Hardcoded strategy registration
//! via if/else blocks in gridbot.rs."
//!
//! Provides:
//! - [`StrategyEntry`]: a strategy bundled with its weight and enabled flag
//! - [`StrategyRegistryBuilder`]: accumulates entries → builds `StrategyManager`
//!
//! PR #98: `build()` now calls `wma_engine.register_strategy()` for every
//! enabled strategy so WMA has a performance-tracking slot from the first tick.
//!
//! PR #99 Commit 3a: `build_with_confidence(ctx, threshold)` added —
//! config-driven path that passes `wma_confidence_threshold` from TOML
//! all the way into `WMAConsensusEngine::with_min_confidence()`.
//!
//! ## Usage
//!
//! ```rust,ignore
//! // Legacy path (tests, non-config callers) — keeps historic 0.65 gate:
//! let (manager, weights) = StrategyRegistryBuilder::new()
//!     .add(grid_rebalancer, 0.40)
//!     .add_if(momentum_enabled, momentum_strategy, 0.20)
//!     .build(analytics_context);
//!
//! // Config-driven path (GridBot::new) — passes TOML threshold:
//! let (manager, weights) = StrategyRegistryBuilder::new()
//!     .add(grid_rebalancer, 0.40)
//!     .add_if(momentum_enabled, momentum_strategy, 0.20)
//!     .build_with_confidence(analytics_context, config.strategies.wma_confidence_threshold);
//! ```

use crate::strategies::{AnalyticsContext, Strategy, StrategyManager};
use log::info;

// ══════════════════════════════════════════════════════════════════════
// STRATEGY ENTRY
// ══════════════════════════════════════════════════════════════════════

/// A strategy bundled with its configured weight and enabled state.
///
/// Weights seed WMA initial importance. A weight of `0.0` marks
/// non-signal strategies (e.g., SmartFeeFilter).
pub struct StrategyEntry {
    pub strategy: Box<dyn Strategy>,
    pub weight: f64,
    pub enabled: bool,
}

impl StrategyEntry {
    /// Create a new enabled entry with the given weight.
    pub fn new(strategy: impl Strategy + 'static, weight: f64) -> Self {
        Self {
            strategy: Box::new(strategy),
            weight,
            enabled: true,
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
            "StrategyEntry {{ name: {:?}, weight: {:.2}, enabled: {} }}",
            self.name(),
            self.weight,
            self.enabled,
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
/// PR #98: both `build()` and `build_with_confidence()` register every
/// enabled strategy with `wma_engine` so the WMA performance tracker has
/// a slot ready before the first tick.
pub struct StrategyRegistryBuilder {
    entries: Vec<StrategyEntry>,
}

impl StrategyRegistryBuilder {
    pub fn new() -> Self {
        Self {
            entries: Vec::new(),
        }
    }

    /// Register a strategy unconditionally (e.g., GridRebalancer — always active).
    pub fn add(mut self, strategy: impl Strategy + 'static, weight: f64) -> Self {
        let entry = StrategyEntry::new(strategy, weight);
        info!(
            "[REGISTRY] Registered: {} (weight={:.2})",
            entry.name(),
            weight,
        );
        self.entries.push(entry);
        self
    }

    /// Register a strategy only if `enabled` is `true` (config-driven).
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
    ///
    /// PR #98: each enabled strategy is registered with `manager.wma_engine`
    /// so the dynamic weight tracker has a slot from the very first tick.
    pub fn build(self, ctx: AnalyticsContext) -> (StrategyManager, Vec<f64>) {
        self.build_inner(ctx, None)
    }

    /// Consume the builder and produce a configured `StrategyManager` with a
    /// config-driven WMA confidence gate.
    ///
    /// PR #99 Commit 3a: this is the canonical path for `GridBot::new()`.
    /// Pass `config.strategies.wma_confidence_threshold` as `wma_confidence`.
    /// The value is already validated in `[0.0, 1.0]` by
    /// `StrategiesConfig::validate()` — no re-validation needed here.
    ///
    /// Returns `(manager, weights)` — identical contract to `build()`.
    pub fn build_with_confidence(
        self,
        ctx: AnalyticsContext,
        wma_confidence: f64,
    ) -> (StrategyManager, Vec<f64>) {
        self.build_inner(ctx, Some(wma_confidence))
    }

    // ───────────────────────────────────────────────────────────────────────
    // Private implementation shared by both public builders.
    // `wma_confidence = None`  → StrategyManager::new()          (0.65 gate)
    // `wma_confidence = Some(t)` → StrategyManager::new_with_confidence(t)
    // ───────────────────────────────────────────────────────────────────────
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

        for mut entry in self.entries {
            if entry.enabled {
                let name = entry.strategy.name().to_string();
                weights.push(entry.weight);
                entry.strategy.attach_analytics(manager.context.clone());
                // PR #98: register with WMA before pushing so the engine has
                // a performance slot from the first analyze_all() call.
                manager.wma_engine.register_strategy(name.clone());
                info!("[REGISTRY] Attached: {} (WMA registered, weight={:.2})", name, entry.weight);
                manager.strategies.push(entry.strategy);
            }
        }

        info!(
            "[REGISTRY] Built StrategyManager with {} strategies (WMA conf_gate={:.2})",
            manager.strategies.len(),
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

    /// build() must register every enabled strategy with wma_engine.
    #[test]
    fn test_registry_build_registers_wma_slots() {
        let gr = GridRebalancer::new(GridRebalancerConfig::default()).unwrap();
        let builder = StrategyRegistryBuilder::new().add(gr, 0.40);
        let ctx = AnalyticsContext::default();
        let (manager, _) = builder.build(ctx);
        let perf = manager.wma_engine.get_performance("GridRebalancer");
        assert!(perf.is_some(), "WMA must have a slot for GridRebalancer after build()");
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
        // WMA slot must still be registered
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
}
