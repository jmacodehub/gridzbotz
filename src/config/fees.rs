//! ═══════════════════════════════════════════════════════════════════════════
//! 💰 FEES CONFIG — Unified Fee Management for GridzBotz
//!
//! PR #75 — Single Source of Truth for All Fee-Related Parameters
//!
//! This module provides the canonical `FeesConfig` struct that centralizes
//! all fee-related configuration. Previously, fees were scattered across:
//!
//! - `engine.rs`: hardcoded `maker_fee_bps = 2.0`, `taker_fee_bps = 4.0`
//! - `paper_trader.rs`: `DEFAULT_MAKER_FEE = 0.0002` (fraction)
//! - `fee_filter.rs`: `maker_fee_percent: 0.02` (percentage)
//! - `grid_rebalancer.rs`: implicit min_spread thresholds per regime
//!
//! Now all consumers read from `config.fees.*` with type-safe converters.
//!
//! ## Unit Convention
//!
//! **BPS (Basis Points) is the canonical unit in config.**
//!
//! - 1 BPS = 0.01% = 0.0001 (fraction)
//! - 100 BPS = 1%
//!
//! Conversion helpers eliminate unit confusion:
//! - `maker_fee_fraction()` → 0.0002 (for multiplication in paper_trader)
//! - `maker_fee_percent()` → 0.02 (for fee_filter percentage math)
//!
//! ## TOML Example
//!
//! ```toml
//! [fees]
//! maker_fee_bps = 2.0         # 0.02% — standard Solana DEX maker fee
//! taker_fee_bps = 4.0         # 0.04% — standard Solana DEX taker fee
//! slippage_bps = 5.0          # 0.05% — expected execution slippage
//! min_profit_multiplier = 2.0 # Require 2x round-trip costs for profitability
//! enable_smart_filter = false # Enable SmartFeeFilter (opt-in)
//! market_impact_coefficient = 0.01  # Impact per SOL of position
//! ```
//!
//! March 2026 — V1.0 LFG 🚀
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
// DEFAULT VALUE FUNCTIONS (for serde)
// ═══════════════════════════════════════════════════════════════════════════

fn default_maker_fee_bps() -> f64 { 2.0 }
fn default_taker_fee_bps() -> f64 { 4.0 }
fn default_slippage_bps() -> f64 { 5.0 }
fn default_min_profit_multiplier() -> f64 { 2.0 }
fn default_market_impact_coefficient() -> f64 { 0.01 }

// ═══════════════════════════════════════════════════════════════════════════
// FEES CONFIG STRUCT
// ═══════════════════════════════════════════════════════════════════════════

/// Unified fee configuration for all trading operations.
///
/// This struct is the **single source of truth** for fee-related parameters.
/// All trading components (engine, strategies, filters) should read from this
/// config rather than using hardcoded values.
///
/// ## Unit: Basis Points (BPS)
///
/// All fee values are stored in BPS for consistency with industry standards:
/// - 1 BPS = 0.01% = 0.0001 (as a fraction)
/// - Jupiter/Raydium typically charge 2-4 BPS maker, 4-10 BPS taker
///
/// Use the conversion methods to get the value in your preferred unit:
/// - `maker_fee_fraction()` → for direct multiplication (e.g., `amount * fee`)
/// - `maker_fee_percent()` → for display or percentage-based math
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeesConfig {
    /// Maker fee in basis points (1 BPS = 0.01%).
    ///
    /// Maker orders add liquidity to the order book and typically receive
    /// lower fees. On Solana DEXs like Jupiter aggregating Orca/Raydium,
    /// maker fees are typically 2-4 BPS.
    ///
    /// Default: 2.0 BPS (0.02%)
    #[serde(default = "default_maker_fee_bps")]
    pub maker_fee_bps: f64,

    /// Taker fee in basis points (1 BPS = 0.01%).
    ///
    /// Taker orders remove liquidity and typically pay higher fees.
    /// On Solana DEXs, taker fees are typically 4-10 BPS.
    ///
    /// Default: 4.0 BPS (0.04%)
    #[serde(default = "default_taker_fee_bps")]
    pub taker_fee_bps: f64,

    /// Expected execution slippage in basis points.
    ///
    /// This is the typical price movement between quote and execution.
    /// Used for profitability calculations and trade filtering.
    /// Does NOT affect the actual slippage tolerance sent to Jupiter
    /// (that's in ExecutionConfig.max_slippage_bps).
    ///
    /// Default: 5.0 BPS (0.05%)
    #[serde(default = "default_slippage_bps")]
    pub slippage_bps: f64,

    /// Minimum profit multiplier over total round-trip costs.
    ///
    /// A trade is only considered profitable if:
    /// `expected_profit >= round_trip_costs * min_profit_multiplier`
    ///
    /// Where round_trip_costs = maker_fee + taker_fee + (2 * slippage)
    ///
    /// Higher values are more conservative (fewer but higher-quality trades).
    /// GIGA testing showed 2.0x is optimal for SOL/USDC grid trading.
    ///
    /// Default: 2.0 (require 2x costs to trade)
    #[serde(default = "default_min_profit_multiplier")]
    pub min_profit_multiplier: f64,

    /// Enable the SmartFeeFilter for trade gating.
    ///
    /// When enabled, trades are filtered through SmartFeeFilter which
    /// considers regime-adjusted fees, market impact, and profitability.
    /// Proven +50% ROI improvement in battle royale testing.
    ///
    /// Default: false (opt-in — enable after validating in paper mode)
    #[serde(default)]
    pub enable_smart_filter: bool,

    /// Market impact coefficient per SOL of position size.
    ///
    /// Larger positions move the market more. This coefficient estimates
    /// additional slippage as: `market_impact = coefficient * position_sol`
    ///
    /// For SOL/USDC on Jupiter with typical liquidity:
    /// - 0.01 = 1 BPS additional impact per SOL
    /// - A 10 SOL position adds ~10 BPS of expected slippage
    ///
    /// Default: 0.01
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

impl FeesConfig {
    // ═══════════════════════════════════════════════════════════════════════
    // BPS → FRACTION CONVERTERS (for paper_trader.rs style multiplication)
    // ═══════════════════════════════════════════════════════════════════════

    /// Convert maker fee from BPS to fraction for direct multiplication.
    ///
    /// Example: 2.0 BPS → 0.0002
    ///
    /// Usage: `let fee = amount * config.fees.maker_fee_fraction();`
    #[inline]
    pub fn maker_fee_fraction(&self) -> f64 {
        self.maker_fee_bps / 10_000.0
    }

    /// Convert taker fee from BPS to fraction for direct multiplication.
    ///
    /// Example: 4.0 BPS → 0.0004
    #[inline]
    pub fn taker_fee_fraction(&self) -> f64 {
        self.taker_fee_bps / 10_000.0
    }

    /// Convert slippage from BPS to fraction for direct multiplication.
    ///
    /// Example: 5.0 BPS → 0.0005
    #[inline]
    pub fn slippage_fraction(&self) -> f64 {
        self.slippage_bps / 10_000.0
    }

    // ═══════════════════════════════════════════════════════════════════════
    // BPS → PERCENT CONVERTERS (for fee_filter.rs style percentage math)
    // ═══════════════════════════════════════════════════════════════════════

    /// Convert maker fee from BPS to percentage.
    ///
    /// Example: 2.0 BPS → 0.02 (meaning 0.02%)
    ///
    /// Usage in percentage math: `spread_pct > maker_fee_percent + taker_fee_percent`
    #[inline]
    pub fn maker_fee_percent(&self) -> f64 {
        self.maker_fee_bps / 100.0
    }

    /// Convert taker fee from BPS to percentage.
    ///
    /// Example: 4.0 BPS → 0.04 (meaning 0.04%)
    #[inline]
    pub fn taker_fee_percent(&self) -> f64 {
        self.taker_fee_bps / 100.0
    }

    /// Convert slippage from BPS to percentage.
    ///
    /// Example: 5.0 BPS → 0.05 (meaning 0.05%)
    #[inline]
    pub fn slippage_percent(&self) -> f64 {
        self.slippage_bps / 100.0
    }

    // ═══════════════════════════════════════════════════════════════════════
    // DERIVED CALCULATIONS
    // ═══════════════════════════════════════════════════════════════════════

    /// Calculate total round-trip cost in BPS.
    ///
    /// Round-trip = entry (taker) + exit (maker) + slippage both ways
    ///
    /// For a grid bot:
    /// - Entry is typically taker (crossing spread)
    /// - Exit is typically maker (posting limit order)
    /// - Slippage occurs on both legs
    #[inline]
    pub fn round_trip_cost_bps(&self) -> f64 {
        self.maker_fee_bps + self.taker_fee_bps + (self.slippage_bps * 2.0)
    }

    /// Calculate total round-trip cost as a fraction.
    #[inline]
    pub fn round_trip_cost_fraction(&self) -> f64 {
        self.round_trip_cost_bps() / 10_000.0
    }

    /// Calculate total round-trip cost as a percentage.
    #[inline]
    pub fn round_trip_cost_percent(&self) -> f64 {
        self.round_trip_cost_bps() / 100.0
    }

    /// Calculate minimum profitable spread in BPS.
    ///
    /// This is the minimum price movement required to cover costs
    /// and meet the profit multiplier requirement.
    ///
    /// `min_spread = round_trip_cost * min_profit_multiplier`
    #[inline]
    pub fn min_profitable_spread_bps(&self) -> f64 {
        self.round_trip_cost_bps() * self.min_profit_multiplier
    }

    /// Calculate minimum profitable spread as a percentage.
    #[inline]
    pub fn min_profitable_spread_percent(&self) -> f64 {
        self.min_profitable_spread_bps() / 100.0
    }

    /// Estimate market impact for a given position size in SOL.
    ///
    /// Returns additional expected slippage in BPS.
    #[inline]
    pub fn estimate_market_impact_bps(&self, position_sol: f64) -> f64 {
        self.market_impact_coefficient * position_sol * 100.0 // coefficient is per-SOL, convert to BPS
    }

    /// Calculate total expected cost for a trade including market impact.
    ///
    /// Returns total cost in BPS for a single-leg trade.
    pub fn total_trade_cost_bps(&self, position_sol: f64, is_taker: bool) -> f64 {
        let base_fee = if is_taker { self.taker_fee_bps } else { self.maker_fee_bps };
        let impact = self.estimate_market_impact_bps(position_sol);
        base_fee + self.slippage_bps + impact
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values_match_current_hardcodes() {
        let config = FeesConfig::default();
        
        // These MUST match the values currently hardcoded in engine.rs
        assert_eq!(config.maker_fee_bps, 2.0, "maker_fee_bps must match engine.rs hardcode");
        assert_eq!(config.taker_fee_bps, 4.0, "taker_fee_bps must match engine.rs hardcode");
        assert_eq!(config.slippage_bps, 5.0, "slippage_bps must match paper_trader.rs DEFAULT_SLIPPAGE");
        assert_eq!(config.min_profit_multiplier, 2.0);
        assert!(!config.enable_smart_filter, "smart filter must be opt-in");
    }

    #[test]
    fn test_fraction_conversions() {
        let config = FeesConfig::default();
        
        // 2 BPS = 0.0002
        assert!((config.maker_fee_fraction() - 0.0002).abs() < 1e-10);
        // 4 BPS = 0.0004
        assert!((config.taker_fee_fraction() - 0.0004).abs() < 1e-10);
        // 5 BPS = 0.0005
        assert!((config.slippage_fraction() - 0.0005).abs() < 1e-10);
    }

    #[test]
    fn test_percent_conversions() {
        let config = FeesConfig::default();
        
        // 2 BPS = 0.02%
        assert!((config.maker_fee_percent() - 0.02).abs() < 1e-10);
        // 4 BPS = 0.04%
        assert!((config.taker_fee_percent() - 0.04).abs() < 1e-10);
        // 5 BPS = 0.05%
        assert!((config.slippage_percent() - 0.05).abs() < 1e-10);
    }

    #[test]
    fn test_round_trip_cost() {
        let config = FeesConfig::default();
        
        // Round-trip = maker (2) + taker (4) + slippage*2 (10) = 16 BPS
        assert_eq!(config.round_trip_cost_bps(), 16.0);
        assert!((config.round_trip_cost_fraction() - 0.0016).abs() < 1e-10);
        assert!((config.round_trip_cost_percent() - 0.16).abs() < 1e-10);
    }

    #[test]
    fn test_min_profitable_spread() {
        let config = FeesConfig::default();
        
        // Min spread = round_trip (16) * multiplier (2.0) = 32 BPS
        assert_eq!(config.min_profitable_spread_bps(), 32.0);
        assert!((config.min_profitable_spread_percent() - 0.32).abs() < 1e-10);
    }

    #[test]
    fn test_market_impact_estimation() {
        let config = FeesConfig::default();
        
        // Default coefficient is 0.01, so 10 SOL = 10 BPS impact
        let impact = config.estimate_market_impact_bps(10.0);
        assert!((impact - 10.0).abs() < 1e-10);
    }

    #[test]
    fn test_total_trade_cost() {
        let config = FeesConfig::default();
        
        // Taker 1 SOL: taker(4) + slippage(5) + impact(1) = 10 BPS
        let cost = config.total_trade_cost_bps(1.0, true);
        assert!((cost - 10.0).abs() < 1e-10);
        
        // Maker 1 SOL: maker(2) + slippage(5) + impact(1) = 8 BPS
        let cost = config.total_trade_cost_bps(1.0, false);
        assert!((cost - 8.0).abs() < 1e-10);
    }

    #[test]
    fn test_serde_round_trip() {
        let original = FeesConfig {
            maker_fee_bps: 3.5,
            taker_fee_bps: 7.0,
            slippage_bps: 10.0,
            min_profit_multiplier: 2.5,
            enable_smart_filter: true,
            market_impact_coefficient: 0.02,
        };
        
        let json = serde_json::to_string(&original).unwrap();
        let restored: FeesConfig = serde_json::from_str(&json).unwrap();
        
        assert_eq!(original.maker_fee_bps, restored.maker_fee_bps);
        assert_eq!(original.taker_fee_bps, restored.taker_fee_bps);
        assert_eq!(original.slippage_bps, restored.slippage_bps);
        assert_eq!(original.min_profit_multiplier, restored.min_profit_multiplier);
        assert_eq!(original.enable_smart_filter, restored.enable_smart_filter);
        assert_eq!(original.market_impact_coefficient, restored.market_impact_coefficient);
    }

    #[test]
    fn test_serde_defaults_on_missing_fields() {
        // Simulate a TOML with only some fields specified
        let json = r#"{"maker_fee_bps": 3.0}"#;
        let config: FeesConfig = serde_json::from_str(json).unwrap();
        
        assert_eq!(config.maker_fee_bps, 3.0); // specified
        assert_eq!(config.taker_fee_bps, 4.0); // default
        assert_eq!(config.slippage_bps, 5.0); // default
        assert!(!config.enable_smart_filter);  // default false
    }
}
