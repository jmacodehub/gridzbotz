//! ═══════════════════════════════════════════════════════════════════════════
//! FEES CONFIG V1.0 — Single Source of Truth for All Fee Parameters
//!
//! PR #75 — Phase 3: FeesConfig Foundation
//!
//! Centralizes maker/taker fees, slippage, and profit thresholds that were
//! previously hardcoded across 4+ files in 3 different unit systems.
//!
//! Canonical unit: **Basis Points (BPS)** — 1 bps = 0.01%
//! Conversion helpers provided for all consumers:
//!   - fraction()  → 0.0002  (for paper_trader.rs multiplication)
//!   - percent()   → 0.02    (for fee_filter.rs percentage math)
//!
//! HARDCODE LOCATIONS THIS REPLACES:
//!   1. engine.rs:        maker_fee_bps = 2.0, taker_fee_bps = 4.0
//!   2. paper_trader.rs:  DEFAULT_MAKER_FEE = 0.0002, DEFAULT_TAKER_FEE = 0.0004
//!   3. fee_filter.rs:    maker_fee_percent: 0.02, taker_fee_percent: 0.04
//!   4. grid_rebalancer:  min_spread per regime (0.05% – 0.15%)
//!
//! March 2026 — V1.0 LFG 🚀
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULTS (match current hardcoded values exactly — zero regression)
// ═══════════════════════════════════════════════════════════════════════════

fn default_maker_fee_bps() -> f64 { 2.0 }
fn default_taker_fee_bps() -> f64 { 4.0 }
fn default_slippage_bps() -> f64 { 5.0 }
fn default_min_profit_multiplier() -> f64 { 2.0 }
fn default_market_impact_coefficient() -> f64 { 0.01 }

// ═══════════════════════════════════════════════════════════════════════════
// FEES CONFIG
// ═══════════════════════════════════════════════════════════════════════════

/// Centralized fee configuration — single source of truth.
///
/// All fee values are specified in **basis points** (1 bps = 0.01%).
/// Use the conversion helpers to get the format each consumer expects:
///
/// ```ignore
/// let fees = FeesConfig::default();
/// assert_eq!(fees.maker_fee_bps, 2.0);           // BPS (raw)
/// assert_eq!(fees.maker_fee_fraction(), 0.0002);  // for multiplication
/// assert_eq!(fees.maker_fee_percent(), 0.02);     // for percentage math
/// ```
///
/// ## TOML Usage
///
/// ```toml
/// [fees]
/// maker_fee_bps = 2.0         # 0.02% — standard Solana DEX maker fee
/// taker_fee_bps = 4.0         # 0.04% — standard Solana DEX taker fee
/// slippage_bps = 5.0          # 0.05% — expected execution slippage
/// min_profit_multiplier = 2.0 # require 2x costs before trading
/// enable_smart_filter = false  # opt-in: wire SmartFeeFilter for trade gating
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FeesConfig {
    /// Maker fee in basis points (1 bps = 0.01%).
    /// Default: 2.0 bps (0.02%) — standard Solana DEX maker fee.
    #[serde(default = "default_maker_fee_bps")]
    pub maker_fee_bps: f64,

    /// Taker fee in basis points.
    /// Default: 4.0 bps (0.04%) — standard Solana DEX taker fee.
    #[serde(default = "default_taker_fee_bps")]
    pub taker_fee_bps: f64,

    /// Expected slippage in basis points.
    /// Default: 5.0 bps (0.05%).
    #[serde(default = "default_slippage_bps")]
    pub slippage_bps: f64,

    /// Minimum profit multiplier over total round-trip costs.
    /// A trade must have expected profit >= costs × this multiplier.
    /// Default: 2.0 (require 2× round-trip costs).
    #[serde(default = "default_min_profit_multiplier")]
    pub min_profit_multiplier: f64,

    /// Enable SmartFeeFilter for trade gating.
    /// When true, SmartFeeFilter is wired as a strategy that gates trades
    /// based on whether expected profit exceeds fee + slippage costs.
    /// Default: false (opt-in to avoid surprise behavior changes).
    #[serde(default)]
    pub enable_smart_filter: bool,

    /// Market impact coefficient per SOL of position size.
    /// Used by SmartFeeFilter to estimate price impact for larger orders.
    /// Default: 0.01 (1% impact per SOL — conservative estimate).
    #[serde(default = "default_market_impact_coefficient")]
    pub market_impact_coefficient: f64,
}

impl Default for FeesConfig {
    fn default() -> Self {
        Self {
            maker_fee_bps: default_maker_fee_bps(),
            taker_fee_bps: default_taker_fee_bps(),
            slippage_bps: default_slippage_bps(),
            min_profit_multiplier: default_min_profit_multiplier(),
            enable_smart_filter: false,
            market_impact_coefficient: default_market_impact_coefficient(),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// CONVERSION HELPERS — zero-cost, compile-time inlineable
// ═══════════════════════════════════════════════════════════════════════════

impl FeesConfig {
    // ── Maker Fee Conversions ────────────────────────────────────────────

    /// Maker fee as a fraction (for direct multiplication).
    /// 2.0 bps → 0.0002
    #[inline]
    pub fn maker_fee_fraction(&self) -> f64 {
        self.maker_fee_bps / 10_000.0
    }

    /// Maker fee as a percentage.
    /// 2.0 bps → 0.02
    #[inline]
    pub fn maker_fee_percent(&self) -> f64 {
        self.maker_fee_bps / 100.0
    }

    // ── Taker Fee Conversions ────────────────────────────────────────────

    /// Taker fee as a fraction (for direct multiplication).
    /// 4.0 bps → 0.0004
    #[inline]
    pub fn taker_fee_fraction(&self) -> f64 {
        self.taker_fee_bps / 10_000.0
    }

    /// Taker fee as a percentage.
    /// 4.0 bps → 0.04
    #[inline]
    pub fn taker_fee_percent(&self) -> f64 {
        self.taker_fee_bps / 100.0
    }

    // ── Slippage Conversions ─────────────────────────────────────────────

    /// Slippage as a fraction (for direct multiplication).
    /// 5.0 bps → 0.0005
    #[inline]
    pub fn slippage_fraction(&self) -> f64 {
        self.slippage_bps / 10_000.0
    }

    /// Slippage as a percentage.
    /// 5.0 bps → 0.05
    #[inline]
    pub fn slippage_percent(&self) -> f64 {
        self.slippage_bps / 100.0
    }

    // ── Aggregate Helpers ────────────────────────────────────────────────

    /// Total round-trip cost in BPS: maker + taker + 2× slippage.
    /// With defaults: 2 + 4 + 10 = 16 bps (0.16%).
    #[inline]
    pub fn round_trip_cost_bps(&self) -> f64 {
        self.maker_fee_bps + self.taker_fee_bps + (self.slippage_bps * 2.0)
    }

    /// Total round-trip cost as a percentage.
    /// With defaults: 0.16%.
    #[inline]
    pub fn round_trip_cost_percent(&self) -> f64 {
        self.round_trip_cost_bps() / 100.0
    }

    /// Minimum spread (in percent) required to be profitable.
    /// Equals round_trip_cost_percent × min_profit_multiplier.
    /// With defaults: 0.16% × 2.0 = 0.32%.
    #[inline]
    pub fn min_profitable_spread_percent(&self) -> f64 {
        self.round_trip_cost_percent() * self.min_profit_multiplier
    }

    /// Minimum spread for a specific market regime.
    /// Returns the fee-derived minimum spread adjusted by a regime multiplier.
    ///
    /// Replaces the hardcoded min_spread values in grid_rebalancer.rs:
    ///   VERY_LOW_VOL  → base_cost × 0.5  (tighter in calm markets)
    ///   LOW_VOL       → base_cost × 0.75
    ///   MEDIUM_VOL    → base_cost × 1.0
    ///   HIGH_VOL      → base_cost × 1.2
    ///   VERY_HIGH_VOL → base_cost × 1.5  (wider in volatile markets)
    pub fn min_spread_for_regime(&self, regime: &str) -> f64 {
        let base_cost_pct = self.round_trip_cost_percent();
        let multiplier = match regime {
            "VERY_LOW_VOL"  => 0.5,
            "LOW_VOL"       => 0.75,
            "MEDIUM_VOL"    => 1.0,
            "HIGH_VOL"      => 1.2,
            "VERY_HIGH_VOL" => 1.5,
            _               => 1.0,
        };
        base_cost_pct * multiplier
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// VALIDATION
// ═══════════════════════════════════════════════════════════════════════════

impl FeesConfig {
    /// Validate fee configuration at startup. Fail fast with actionable errors.
    pub fn validate(&self) -> Result<(), String> {
        if self.maker_fee_bps < 0.0 {
            return Err(format!(
                "fees.maker_fee_bps must be >= 0, got {}. \
                 Check [fees] section in your TOML config.",
                self.maker_fee_bps
            ));
        }
        if self.taker_fee_bps < 0.0 {
            return Err(format!(
                "fees.taker_fee_bps must be >= 0, got {}. \
                 Check [fees] section in your TOML config.",
                self.taker_fee_bps
            ));
        }
        if self.slippage_bps < 0.0 {
            return Err(format!(
                "fees.slippage_bps must be >= 0, got {}. \
                 Check [fees] section in your TOML config.",
                self.slippage_bps
            ));
        }
        if self.min_profit_multiplier < 1.0 {
            return Err(format!(
                "fees.min_profit_multiplier must be >= 1.0, got {}. \
                 Values below 1.0 mean trading at a guaranteed loss.",
                self.min_profit_multiplier
            ));
        }
        if self.maker_fee_bps > 100.0 {
            return Err(format!(
                "fees.maker_fee_bps={} seems too high (>1%). \
                 Value is in basis points: 2.0 = 0.02%. Did you mean {}?",
                self.maker_fee_bps, self.maker_fee_bps / 100.0
            ));
        }
        if self.taker_fee_bps > 100.0 {
            return Err(format!(
                "fees.taker_fee_bps={} seems too high (>1%). \
                 Value is in basis points: 4.0 = 0.04%. Did you mean {}?",
                self.taker_fee_bps, self.taker_fee_bps / 100.0
            ));
        }
        if self.market_impact_coefficient < 0.0 || self.market_impact_coefficient > 1.0 {
            return Err(format!(
                "fees.market_impact_coefficient must be 0.0–1.0, got {}.",
                self.market_impact_coefficient
            ));
        }
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_defaults_match_current_hardcodes() {
        // These defaults MUST match the values currently hardcoded in:
        // - engine.rs: maker_fee_bps=2.0, taker_fee_bps=4.0
        // - paper_trader.rs: DEFAULT_MAKER_FEE=0.0002, DEFAULT_TAKER_FEE=0.0004
        // - fee_filter.rs: maker_fee_percent=0.02, taker_fee_percent=0.04
        let fees = FeesConfig::default();
        assert_eq!(fees.maker_fee_bps, 2.0);
        assert_eq!(fees.taker_fee_bps, 4.0);
        assert_eq!(fees.slippage_bps, 5.0);
        assert_eq!(fees.min_profit_multiplier, 2.0);
        assert!(!fees.enable_smart_filter);
    }

    #[test]
    fn test_maker_fee_conversions() {
        let fees = FeesConfig::default();
        assert!((fees.maker_fee_fraction() - 0.0002).abs() < f64::EPSILON);
        assert!((fees.maker_fee_percent() - 0.02).abs() < f64::EPSILON);
    }

    #[test]
    fn test_taker_fee_conversions() {
        let fees = FeesConfig::default();
        assert!((fees.taker_fee_fraction() - 0.0004).abs() < f64::EPSILON);
        assert!((fees.taker_fee_percent() - 0.04).abs() < f64::EPSILON);
    }

    #[test]
    fn test_slippage_conversions() {
        let fees = FeesConfig::default();
        assert!((fees.slippage_fraction() - 0.0005).abs() < f64::EPSILON);
        assert!((fees.slippage_percent() - 0.05).abs() < f64::EPSILON);
    }

    #[test]
    fn test_round_trip_cost() {
        let fees = FeesConfig::default();
        // maker(2) + taker(4) + 2*slippage(10) = 16 bps
        assert!((fees.round_trip_cost_bps() - 16.0).abs() < f64::EPSILON);
        assert!((fees.round_trip_cost_percent() - 0.16).abs() < f64::EPSILON);
    }

    #[test]
    fn test_min_profitable_spread() {
        let fees = FeesConfig::default();
        // 0.16% round-trip × 2.0 multiplier = 0.32%
        assert!((fees.min_profitable_spread_percent() - 0.32).abs() < f64::EPSILON);
    }

    #[test]
    fn test_min_spread_for_regime_ordering() {
        let fees = FeesConfig::default();
        let vlv = fees.min_spread_for_regime("VERY_LOW_VOL");
        let lv = fees.min_spread_for_regime("LOW_VOL");
        let mv = fees.min_spread_for_regime("MEDIUM_VOL");
        let hv = fees.min_spread_for_regime("HIGH_VOL");
        let vhv = fees.min_spread_for_regime("VERY_HIGH_VOL");

        // More vol = wider spread required
        assert!(vlv < lv);
        assert!(lv < mv);
        assert!(mv < hv);
        assert!(hv < vhv);

        // All in reasonable range (0.01% – 1.0%)
        for spread in [vlv, lv, mv, hv, vhv] {
            assert!(spread > 0.01, "Spread too tight: {}", spread);
            assert!(spread < 1.0, "Spread too wide: {}", spread);
        }
    }

    #[test]
    fn test_custom_fees() {
        let fees = FeesConfig {
            maker_fee_bps: 5.0,
            taker_fee_bps: 10.0,
            slippage_bps: 8.0,
            min_profit_multiplier: 3.0,
            enable_smart_filter: true,
            market_impact_coefficient: 0.02,
        };
        assert!((fees.maker_fee_fraction() - 0.0005).abs() < f64::EPSILON);
        assert!((fees.taker_fee_fraction() - 0.001).abs() < f64::EPSILON);
        // Round trip: 5 + 10 + 16 = 31 bps
        assert!((fees.round_trip_cost_bps() - 31.0).abs() < f64::EPSILON);
    }

    #[test]
    fn test_validate_valid_config() {
        assert!(FeesConfig::default().validate().is_ok());
    }

    #[test]
    fn test_validate_negative_maker_fee() {
        let mut fees = FeesConfig::default();
        fees.maker_fee_bps = -1.0;
        let err = fees.validate().unwrap_err();
        assert!(err.contains("maker_fee_bps"));
    }

    #[test]
    fn test_validate_negative_taker_fee() {
        let mut fees = FeesConfig::default();
        fees.taker_fee_bps = -1.0;
        let err = fees.validate().unwrap_err();
        assert!(err.contains("taker_fee_bps"));
    }

    #[test]
    fn test_validate_too_high_maker_warns() {
        let mut fees = FeesConfig::default();
        fees.maker_fee_bps = 200.0; // 2% — probably a mistake
        let err = fees.validate().unwrap_err();
        assert!(err.contains("too high"));
    }

    #[test]
    fn test_validate_sub_one_multiplier() {
        let mut fees = FeesConfig::default();
        fees.min_profit_multiplier = 0.5; // guaranteed loss
        let err = fees.validate().unwrap_err();
        assert!(err.contains("guaranteed loss"));
    }

    #[test]
    fn test_serde_round_trip() {
        let original = FeesConfig::default();
        let toml_str = toml::to_string(&original).expect("serialize");
        let restored: FeesConfig = toml::from_str(&toml_str).expect("deserialize");
        assert!((original.maker_fee_bps - restored.maker_fee_bps).abs() < f64::EPSILON);
        assert!((original.taker_fee_bps - restored.taker_fee_bps).abs() < f64::EPSILON);
        assert!((original.slippage_bps - restored.slippage_bps).abs() < f64::EPSILON);
    }

    #[test]
    fn test_serde_missing_fields_use_defaults() {
        // Empty TOML → all defaults (existing configs without [fees] work)
        let fees: FeesConfig = toml::from_str("").expect("empty should use defaults");
        assert_eq!(fees.maker_fee_bps, 2.0);
        assert_eq!(fees.taker_fee_bps, 4.0);
        assert_eq!(fees.slippage_bps, 5.0);
    }

    #[test]
    fn test_serde_partial_override() {
        let toml_str = "maker_fee_bps = 3.0";
        let fees: FeesConfig = toml::from_str(toml_str).expect("partial override");
        assert_eq!(fees.maker_fee_bps, 3.0); // overridden
        assert_eq!(fees.taker_fee_bps, 4.0); // default
    }
}
