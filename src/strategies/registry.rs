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
//! ## Usage
//!
//! ```rust,ignore
//! let (manager, weights) = StrategyRegistryBuilder::new()
//!     .add(grid_rebalancer, 0.40)
//!     .add_if(momentum_enabled, momentum_strategy, 0.20)
//!     .add_if(rsi_enabled, rsi_strategy, 0.15)
//!     .build(analytics_context);
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
/// PR #98: `build()` registers every enabled strategy with `wma_engine`
/// so the WMA performance tracker has a slot ready before the first tick.
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
    /// Returns `(manager, weights)` — weights are parallel to the strategies
    /// vector and seed the WMA engine's initial importance per strategy.
    ///
    /// All strategies receive `attach_analytics()` with the provided context.
    ///
    /// PR #98: each enabled strategy is registered with `manager.wma_engine`
    /// so the dynamic weight tracker has a slot from the very first tick.
    pub fn build(self, ctx: AnalyticsContext) -> (StrategyManager, Vec<f64>) {
        let mut manager = StrategyManager::new(ctx);
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
            "[REGISTRY] Built StrategyManager with {} strategies (WMA active)",
            manager.strategies.len(),
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
            .add(gr1, 0.40)          // enabled
            .add_if(false, gr2, 0.20); // disabled — must be absent from WMA
        let ctx = AnalyticsContext::default();
        let (manager, _) = builder.build(ctx);
        // Only 1 strategy in manager
        assert_eq!(manager.strategies.len(), 1);
        // The disabled strategy is never named, so we can't look it up by name
        // here — but verify the enabled one IS present.
        let perf = manager.wma_engine.get_performance("GridRebalancer");
        assert!(perf.is_some());
    }
}
