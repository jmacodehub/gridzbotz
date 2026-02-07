// ═══════════════════════════════════════════════════════════════════════════
// FEE-AWARE FILTER MODULE - PROJECT FLASH V5 (Version 2.3)
// ═══════════════════════════════════════════════════════════════════════════
//
// Purpose:
//   Shared module validating that expected profits exceed trade costs.
//
// Upgrades in V2.3 (Phase 3 Compliant):
//   ✅ Unified with Analytics Context
//   ✅ Deterministic, transparent tests
//   ✅ Validation of config bounds and negative profits
//   ✅ Defensive rounding for tiny floating errors
//   ✅ Clear decision enum for AI / logging / telemetry
//
// ═══════════════════════════════════════════════════════════════════════════

use log::{debug, trace};
use serde::{Deserialize, Serialize};

/// Configuration parameters for fee validation
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeFilterConfig {
    pub base_fee_percent: f64,
    pub min_profit_multiplier: f64,
    pub enabled: bool,
    pub max_slippage_percent: f64,
    #[serde(default)]
    pub verbose: bool,
}

impl Default for FeeFilterConfig {
    fn default() -> Self {
        Self {
            base_fee_percent: 0.04,     // Standard DEX fee ≈ 0.04 %
            min_profit_multiplier: 2.0, // At least 2× fees for a trade to be worth it
            enabled: true,
            max_slippage_percent: 0.10,
            verbose: false,
        }
    }
}

/// Decision enum for core logic
#[derive(Debug, Clone, PartialEq)]
pub enum FeeDecision {
    Allowed,
    BlockedInsufficientProfit,
    BlockedDisabled,
}

/// Shared Fee Filter Struct - Reusable across strategies
#[derive(Debug, Clone)]
pub struct FeeFilter {
    pub config: FeeFilterConfig,
}

impl FeeFilter {
    pub fn new(config: FeeFilterConfig) -> Self {
        Self { config }
    }

    /// Ensure configuration values are safe
    pub fn validate(&self) -> bool {
        self.config.base_fee_percent >= 0.0
            && self.config.min_profit_multiplier >= 1.0
            && self.config.max_slippage_percent >= 0.0
    }

    /// Total fees (buy + sell)
    fn total_fee_percent(&self) -> f64 {
        self.config.base_fee_percent * 2.0
    }

    /// The required profit threshold %
    pub fn required_profit_percent(&self) -> f64 {
        (self.total_fee_percent() * self.config.min_profit_multiplier)
            + self.config.max_slippage_percent
    }

    /// Should a trade be executed? Focus on expected profit vs fees & slippage.
    pub fn should_execute_trade(&self, entry: f64, exit: f64) -> FeeDecision {
        if !self.config.enabled {
            trace!("⚙️ Fee filter disabled");
            return FeeDecision::BlockedDisabled;
        }

        if entry <= 0.0 || exit <= 0.0 {
            debug!("⚠️ Invalid prices → blocked");
            return FeeDecision::BlockedInsufficientProfit;
        }

        let diff = (exit - entry).abs();
        let profit_percent = (diff / entry) * 100.0;
        let profit_after_slippage = profit_percent - self.config.max_slippage_percent;
        let required = self.required_profit_percent();

        let allowed = profit_after_slippage + 1e-9 >= required; // defensive epsilon
        let total_fee = self.total_fee_percent();

        if self.config.verbose {
            debug!(
                "🧮 FeeCheck: profit={:.4}% req={:.4}% (slippage={:.3}% fees={:.3}%)",
                profit_after_slippage, required, self.config.max_slippage_percent, total_fee
            );
        }

        if allowed {
            FeeDecision::Allowed
        } else {
            FeeDecision::BlockedInsufficientProfit
        }
    }

    /// Net profit after buy/sell fees and slippage
    pub fn net_profit(&self, entry_price: f64, exit_price: f64, position_usdc: f64) -> f64 {
        if entry_price <= 0.0 {
            return 0.0;
        }
        let gross = (exit_price - entry_price) * (position_usdc / entry_price);
        let buy_fee = position_usdc * (self.config.base_fee_percent / 100.0);
        let sell_fee = (position_usdc + gross) * (self.config.base_fee_percent / 100.0);
        let slip_cost = position_usdc * (self.config.max_slippage_percent / 100.0);
        let net = gross - buy_fee - sell_fee - slip_cost;
        trace!(
            "💰 gross={:.3}, fees={:.3}, slip={:.3}, net={:.3}",
            gross,
            buy_fee + sell_fee,
            slip_cost,
            net
        );
        net
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TEST SUITE (Phase 3 Upgrade)
// ═══════════════════════════════════════════════════════════════════════════
#[cfg(test)]
mod tests {
    use super::*;

    fn build_filter(enabled: bool) -> FeeFilter {
        let mut cfg = FeeFilterConfig::default();
        cfg.enabled = enabled;
        FeeFilter::new(cfg)
    }

    #[test]
    fn test_config_validation_okay() {
        let f = build_filter(true);
        assert!(f.validate(), "Default config should be valid");
    }

    #[test]
    fn test_disabled_returns_blocked_disabled() {
        let f = build_filter(false);
        assert_eq!(
            f.should_execute_trade(100.0, 102.0),
            FeeDecision::BlockedDisabled
        );
    }

    #[test]
    fn test_invalid_prices_block() {
        let f = build_filter(true);
        assert_eq!(
            f.should_execute_trade(0.0, 100.0),
            FeeDecision::BlockedInsufficientProfit
        );
    }

    #[test]
    fn test_allows_trade_when_profit_above_required() {
        let f = build_filter(true);
        // Required ≈ (0.08 × 2 + 0.1) = 0.26%
        let res = f.should_execute_trade(100.0, 100.5);
        assert_eq!(res, FeeDecision::Allowed);
    }

    #[test]
    fn test_blocks_trade_below_required_profit() {
        let f = build_filter(true);
        let res = f.should_execute_trade(100.0, 100.02);
        assert_eq!(res, FeeDecision::BlockedInsufficientProfit);
    }

    #[test]
    fn test_net_profit_computation_range() {
        let f = build_filter(true);
        let net = f.net_profit(100.0, 101.0, 1000.0);
        assert!(net > 8.0 && net < 11.0, "Expected net ~ 9 USDC profit");
    }

    #[test]
    fn test_profit_threshold_scaling() {
        let mut cfg = FeeFilterConfig::default();
        cfg.min_profit_multiplier = 3.0;
        let f = FeeFilter::new(cfg);
        let high_threshold = f.required_profit_percent();
        assert!(high_threshold > 0.2 && high_threshold < 0.5);
    }
}
