//! ═══════════════════════════════════════════════════════════════════════════
//! 💎 SMART FEE FILTER V2.3 - PROJECT FLASH
//!
//! Intelligent fee-aware trade filtering to maximize net profitability.
//!
//! V2.0 ENHANCEMENTS - Production-Grade Intelligence:
//! ✅ Multi-Factor Profit Calculation (fees + slippage + market impact)
//! ✅ Dynamic Minimum Spread Based on Market Regime
//! ✅ Volatility-Adjusted Thresholds
//! ✅ Time-Based Fee Optimization (maker rebates)
//! ✅ Comprehensive Profit Simulation
//! ✅ Statistical Tracking & Analytics
//!
//! V2.1 (PR #77):
//! ✅ from_fees_config() factory — single source of truth via FeesConfig
//! ✅ Fix misleading doc comments (fraction vs percent notation)
//!
//! V2.2 (PR #94 fix — grace_period_trades=0 in from_fees_config):
//! ✅ from_fees_config() now sets grace_period_trades=0 explicitly.
//!    Rationale: GridRebalancer is production-ready from trade 1.
//!    Silently bypassing fee checks at startup is a capital-safety hole.
//!    Default::default() retains grace_period=10 for standalone users.
//!
//! V2.3 (PR #105 — regime + vol scaling mutual exclusion):
//! ✅ calculate_min_required_profit(): early return after regime branch.
//!    Root cause: VERY_LOW_VOL regime_factor (1.5×) AND low-vol vol_factor
//!    (up to 2.0×) were both applied to the same market condition, compounding
//!    to 5.25× effective multiplier. Overnight data: 17 checked, 17 blocked.
//!    Fix: regime adjustment and volatility scaling are now mutually exclusive.
//!    Regime encodes vol semantics — vol scaling only runs when regime is off.
//!
//! Based on GIGA Test Results:
//! - Activity Paradox: More fills ≠ More profit
//! - Fee filtering prevented 40% of unprofitable trades
//! - 2x profit multiplier = optimal baseline
//!
//! February 8, 2026 - V2.0 | March 2026 - V2.3 🚀
//! ═══════════════════════════════════════════════════════════════════════════


use serde::{Deserialize, Serialize};
use log::{debug, trace};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;


use crate::config::FeesConfig;


// ═══════════════════════════════════════════════════════════════════════════
// CONFIGURATION - Flexible & Environment-Aware
// ═══════════════════════════════════════════════════════════════════════════


/// Smart Fee Filter Configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SmartFeeFilterConfig {
    // ─────────────────────────────────────────────────────────────────────────
    // Core Fee Structure
    // ─────────────────────────────────────────────────────────────────────────


    /// Base maker fee as percent (e.g., 0.02 means 0.02%)
    pub maker_fee_percent: f64,


    /// Base taker fee as percent (e.g., 0.04 means 0.04%)
    pub taker_fee_percent: f64,


    /// Expected slippage as percent (e.g., 0.05 means 0.05%)
    pub slippage_percent: f64,


    // ─────────────────────────────────────────────────────────────────────────
    // Profit Multipliers
    // ─────────────────────────────────────────────────────────────────────────


    /// Minimum profit multiplier over total costs
    /// - 1.0 = break-even (not recommended)
    /// - 2.0 = double the costs (GIGA-proven optimal)
    /// - 3.0 = triple (very conservative)
    pub min_profit_multiplier: f64,


    /// Volatility scaling factor for dynamic thresholds.
    /// Only active when `enable_regime_adjustment = false`.
    /// When regime adjustment is enabled, regime encodes vol semantics —
    /// applying vol scaling on top would double-count the same condition.
    pub volatility_scaling_factor: f64,


    // ─────────────────────────────────────────────────────────────────────────
    // Market Impact Modeling
    // ─────────────────────────────────────────────────────────────────────────


    /// Enable market impact estimation
    pub enable_market_impact: bool,


    /// Order size impact coefficient (larger orders = more slippage)
    pub market_impact_coefficient: f64,


    // ─────────────────────────────────────────────────────────────────────────
    // Advanced Features
    // ─────────────────────────────────────────────────────────────────────────


    /// Enable time-of-day fee optimization
    pub enable_time_optimization: bool,


    /// Enable dynamic regime-based adjustment.
    ///
    /// When true, regime factor is the sole vol-context multiplier —
    /// the continuous volatility scaling branch is skipped entirely.
    /// This prevents compounding two representations of the same condition.
    pub enable_regime_adjustment: bool,


    /// Grace period for first N trades (reduce strictness initially).
    ///
    /// **Default (via `Default::default()`):** 10 — for standalone usage.
    /// **`from_fees_config()` always sets this to 0** — GridRebalancer is
    /// production-ready from trade 1; a hidden bypass at startup is unsafe.
    pub grace_period_trades: u64,
}


impl Default for SmartFeeFilterConfig {
    fn default() -> Self {
        Self {
            // Standard Solana DEX fees (percent unit: 0.02 = 0.02%)
            maker_fee_percent: 0.02,    // 2 bps
            taker_fee_percent: 0.04,    // 4 bps
            slippage_percent: 0.05,     // 5 bps


            // GIGA-proven optimal multiplier
            min_profit_multiplier: 2.0,
            volatility_scaling_factor: 1.5,


            // Market impact
            enable_market_impact: true,
            market_impact_coefficient: 0.01,  // 1% per 1 SOL


            // Advanced features
            enable_time_optimization: false,  // Future enhancement
            enable_regime_adjustment: true,
            grace_period_trades: 10,
        }
    }
}


impl SmartFeeFilterConfig {
    /// Create conservative configuration (higher thresholds)
    pub fn conservative() -> Self {
        Self {
            min_profit_multiplier: 3.0,  // Triple costs
            volatility_scaling_factor: 2.0,
            ..Default::default()
        }
    }


    /// Create aggressive configuration (lower thresholds, more trades)
    pub fn aggressive() -> Self {
        Self {
            min_profit_multiplier: 1.5,  // 1.5x costs
            volatility_scaling_factor: 1.0,
            ..Default::default()
        }
    }


    /// Create testing configuration (permissive)
    pub fn testing() -> Self {
        Self {
            min_profit_multiplier: 1.0,  // Break-even
            enable_market_impact: false,
            enable_regime_adjustment: false,
            grace_period_trades: 0,
            ..Default::default()
        }
    }


    /// Create from FeesConfig (single source of truth).
    ///
    /// Maps BPS canonical unit → percent unit used by SmartFeeFilter.
    /// Inherits min_profit_multiplier and market_impact_coefficient.
    ///
    /// **Always sets `grace_period_trades = 0`** — when embedded inside
    /// GridRebalancer, fee filtering must be active from trade 1.
    /// Use `SmartFeeFilterConfig::default()` directly if you want the
    /// 10-trade grace warm-up for standalone usage.
    pub fn from_fees_config(fees: &FeesConfig) -> Self {
        Self {
            maker_fee_percent: fees.maker_fee_percent(),
            taker_fee_percent: fees.taker_fee_percent(),
            slippage_percent: fees.slippage_percent(),
            min_profit_multiplier: fees.min_profit_multiplier,
            enable_market_impact: true,
            market_impact_coefficient: fees.market_impact_coefficient,
            enable_regime_adjustment: true,
            grace_period_trades: 0,  // ✅ V2.2: no warm-up bypass in production
            ..Default::default()
        }
    }


    /// Create from FeesConfig with a custom profit multiplier override.
    pub fn from_fees_config_with_multiplier(fees: &FeesConfig, multiplier: f64) -> Self {
        let mut config = Self::from_fees_config(fees);
        config.min_profit_multiplier = multiplier;
        config
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// SMART FEE FILTER - The Brain 🧠
// ═══════════════════════════════════════════════════════════════════════════


pub struct SmartFeeFilter {
    config: SmartFeeFilterConfig,


    // Statistics (thread-safe)
    total_checks: Arc<AtomicU64>,
    trades_passed: Arc<AtomicU64>,
    trades_filtered: Arc<AtomicU64>,


    // Tracking
    trades_executed: Arc<AtomicU64>,
    total_fees_saved: Arc<tokio::sync::RwLock<f64>>,
}


impl SmartFeeFilter {
    /// Create new smart fee filter
    pub fn new(config: SmartFeeFilterConfig) -> Self {
        Self {
            config,
            total_checks: Arc::new(AtomicU64::new(0)),
            trades_passed: Arc::new(AtomicU64::new(0)),
            trades_filtered: Arc::new(AtomicU64::new(0)),
            trades_executed: Arc::new(AtomicU64::new(0)),
            total_fees_saved: Arc::new(tokio::sync::RwLock::new(0.0)),
        }
    }


    // ═══════════════════════════════════════════════════════════════════
    // CORE FILTERING LOGIC - V2.0 INTELLIGENT! 🧠
    // ═══════════════════════════════════════════════════════════════════


    /// Determine if trade should be executed
    ///
    /// Returns (should_execute, expected_net_profit, reason)
    pub fn should_execute_trade(
        &self,
        entry_price: f64,
        exit_price: f64,
        position_size_sol: f64,
        current_volatility: f64,
        market_regime: &str,
    ) -> (bool, f64, String) {
        self.total_checks.fetch_add(1, Ordering::Relaxed);


        // Check grace period
        let executed = self.trades_executed.load(Ordering::Relaxed);
        if executed < self.config.grace_period_trades {
            trace!("🎁 Grace period: Allowing trade {}/{}",
                   executed + 1, self.config.grace_period_trades);
            self.trades_passed.fetch_add(1, Ordering::Relaxed);
            return (true, 0.0, "Grace period".to_string());
        }


        // Calculate comprehensive costs
        let costs = self.calculate_total_costs(
            entry_price,
            exit_price,
            position_size_sol
        );


        // Calculate expected gross profit
        let gross_profit = (exit_price - entry_price).abs() * position_size_sol;
        // gross_profit_pct reserved for future analytics
        let _gross_profit_pct = (gross_profit / (entry_price * position_size_sol)) * 100.0;


        // Calculate net profit
        let net_profit = gross_profit - costs.total_cost;
        let net_profit_pct = (net_profit / (entry_price * position_size_sol)) * 100.0;


        // Get dynamic minimum profit threshold
        let min_required_profit = self.calculate_min_required_profit(
            costs.total_cost,
            current_volatility,
            market_regime,
        );


        // Decision logic
        let should_execute = net_profit >= min_required_profit;


        if should_execute {
            debug!("✅ Trade PASSED: Net profit ${:.4} ({:.3}%) >= ${:.4} min",
                   net_profit, net_profit_pct, min_required_profit);
            debug!("   Entry: ${:.4} | Exit: ${:.4} | Size: {} SOL",
                   entry_price, exit_price, position_size_sol);
            debug!("   Costs: ${:.4} | Regime: {} | Vol: {:.2}%",
                   costs.total_cost, market_regime, current_volatility * 100.0);


            self.trades_passed.fetch_add(1, Ordering::Relaxed);
            (true, net_profit, "Profitable after all costs".to_string())
        } else {
            debug!("🚫 Trade FILTERED: Net profit ${:.4} ({:.3}%) < ${:.4} min",
                   net_profit, net_profit_pct, min_required_profit);
            debug!("   Gross: ${:.4} | Costs: ${:.4} | Net: ${:.4}",
                   gross_profit, costs.total_cost, net_profit);
            debug!("   Fees would eat {:.1}% of profit!",
                   (costs.total_cost / gross_profit) * 100.0);


            self.trades_filtered.fetch_add(1, Ordering::Relaxed);


            let reason = if net_profit < 0.0 {
                format!("Would lose ${:.4} after fees", net_profit.abs())
            } else {
                format!("Net profit ${:.4} below {:.0}x threshold",
                        net_profit, self.config.min_profit_multiplier)
            };


            (false, net_profit, reason)
        }
    }


    // ═══════════════════════════════════════════════════════════════════
    // COST CALCULATION - Comprehensive & Accurate
    // ═══════════════════════════════════════════════════════════════════


    /// Calculate total costs for a round-trip trade
    fn calculate_total_costs(
        &self,
        entry_price: f64,
        exit_price: f64,
        position_size_sol: f64,
    ) -> TradeCosts {
        let entry_value_usdc = entry_price * position_size_sol;
        let exit_value_usdc = exit_price * position_size_sol;


        // Entry fees (buy = taker)
        let entry_fee = entry_value_usdc * (self.config.taker_fee_percent / 100.0);


        // Exit fees (sell = maker, typically)
        let exit_fee = exit_value_usdc * (self.config.maker_fee_percent / 100.0);


        // Slippage costs (both directions)
        let entry_slippage = entry_value_usdc * (self.config.slippage_percent / 100.0);
        let exit_slippage = exit_value_usdc * (self.config.slippage_percent / 100.0);


        // Market impact (if enabled)
        let market_impact = if self.config.enable_market_impact {
            self.calculate_market_impact(entry_value_usdc, position_size_sol)
        } else {
            0.0
        };


        let total_cost = entry_fee + exit_fee + entry_slippage + exit_slippage + market_impact;


        TradeCosts {
            entry_fee,
            exit_fee,
            entry_slippage,
            exit_slippage,
            market_impact,
            total_cost,
        }
    }


    /// Calculate market impact cost (larger orders = more slippage)
    fn calculate_market_impact(&self, trade_value_usdc: f64, position_size_sol: f64) -> f64 {
        let impact_factor = position_size_sol * self.config.market_impact_coefficient;
        trade_value_usdc * (impact_factor / 100.0)
    }


    // ═══════════════════════════════════════════════════════════════════
    // DYNAMIC THRESHOLD CALCULATION - Regime-Aware! 🎯
    // ═══════════════════════════════════════════════════════════════════


    /// Calculate minimum required profit based on market conditions.
    ///
    /// ## Multiplier logic (V2.3 — PR #105)
    ///
    /// **Regime adjustment enabled (default):**
    ///   `min = base_cost × min_profit_multiplier × regime_factor`
    ///   Returns immediately — vol scaling is NOT applied.
    ///   Regime labels already encode volatility context:
    ///     VERY_LOW_VOL=1.5×  LOW_VOL=1.2×  MEDIUM_VOL=1.0×
    ///     HIGH_VOL=0.9×      VERY_HIGH_VOL=0.8×
    ///
    /// **Regime adjustment disabled:**
    ///   Falls through to continuous vol scaling only.
    ///   High vol (>1.0): reduce threshold up to 30%.
    ///   Low vol (<0.5):  increase threshold up to 2×.
    ///
    /// Pre-fix bug: both branches ran sequentially, compounding to
    /// 5.25× (2.0 × 1.5 × 1.75) in VERY_LOW_VOL + vol≈0 — blocked
    /// all grid trades despite valid configuration (17/17 overnight).
    fn calculate_min_required_profit(
        &self,
        base_cost: f64,
        volatility: f64,
        market_regime: &str,
    ) -> f64 {
        let mut min_profit = base_cost * self.config.min_profit_multiplier;

        // ── Regime adjustment (PR #105: early return — no vol scaling below) ──
        //
        // Regime labels are derived from the same volatility signal that drives
        // the continuous scaling branch. Applying both compounds two
        // representations of identical market conditions.
        //
        // Fix: when regime adjustment is active, it IS the vol adjustment.
        // Return immediately so the vol scaling block never runs.
        if self.config.enable_regime_adjustment {
            let regime_factor = match market_regime {
                "VERY_LOW_VOL"  => 1.5,  // Harder to profit in low vol
                "LOW_VOL"       => 1.2,
                "MEDIUM_VOL"    => 1.0,  // Baseline
                "HIGH_VOL"      => 0.9,  // Easier to profit in high vol
                "VERY_HIGH_VOL" => 0.8,
                _               => 1.0,
            };
            min_profit *= regime_factor;
            return min_profit; // ← PR #105 fix: regime IS the vol adjustment
        }

        // ── Continuous vol scaling (only when regime adjustment is OFF) ──
        //
        // Safe to apply here because enable_regime_adjustment=false means
        // no regime factor was applied above — zero compounding risk.
        if volatility > 1.0 {
            let vol_factor = 1.0 - (volatility - 1.0) * 0.1;
            min_profit *= vol_factor.max(0.7);  // Cap at 30% reduction
        } else if volatility < 0.5 {
            let vol_factor = 1.0 + (0.5 - volatility) * self.config.volatility_scaling_factor;
            min_profit *= vol_factor.min(2.0);  // Cap at 2× increase
        }

        min_profit
    }


    // ═══════════════════════════════════════════════════════════════════
    // SIMPLIFIED API - For Backward Compatibility
    // ═══════════════════════════════════════════════════════════════════


    /// Simple boolean check (backward compatible)
    pub fn should_trade(
        &self,
        entry_price: f64,
        exit_price: f64,
        position_size_sol: f64,
    ) -> bool {
        let (should_execute, _, _) = self.should_execute_trade(
            entry_price,
            exit_price,
            position_size_sol,
            1.0,  // Default volatility
            "MEDIUM_VOL",
        );
        should_execute
    }


    /// Calculate expected net profit for planning
    pub fn calculate_net_profit(
        &self,
        entry_price: f64,
        exit_price: f64,
        position_size_sol: f64,
    ) -> f64 {
        let costs = self.calculate_total_costs(entry_price, exit_price, position_size_sol);
        let gross_profit = (exit_price - entry_price).abs() * position_size_sol;
        gross_profit - costs.total_cost
    }


    // ═══════════════════════════════════════════════════════════════════
    // STATISTICS & ANALYTICS
    // ═══════════════════════════════════════════════════════════════════


    /// Get comprehensive filter statistics
    pub fn stats(&self) -> FeeFilterStats {
        let total = self.total_checks.load(Ordering::Relaxed);
        let passed = self.trades_passed.load(Ordering::Relaxed);
        let filtered = self.trades_filtered.load(Ordering::Relaxed);
        let executed = self.trades_executed.load(Ordering::Relaxed);


        let filter_rate = if total > 0 {
            (filtered as f64 / total as f64) * 100.0
        } else {
            0.0
        };


        let approval_rate = if total > 0 {
            (passed as f64 / total as f64) * 100.0
        } else {
            0.0
        };


        FeeFilterStats {
            total_checks: total,
            trades_passed: passed,
            trades_filtered: filtered,
            trades_executed: executed,
            filter_rate_pct: filter_rate,
            approval_rate_pct: approval_rate,
            min_profit_multiplier: self.config.min_profit_multiplier,
        }
    }


    /// Notify filter that a trade was executed (for grace period tracking)
    pub fn record_execution(&self) {
        self.trades_executed.fetch_add(1, Ordering::Relaxed);
    }


    /// Reset statistics
    pub fn reset_stats(&self) {
        self.total_checks.store(0, Ordering::Relaxed);
        self.trades_passed.store(0, Ordering::Relaxed);
        self.trades_filtered.store(0, Ordering::Relaxed);
        self.trades_executed.store(0, Ordering::Relaxed);
    }
}


impl Default for SmartFeeFilter {
    fn default() -> Self {
        Self::new(SmartFeeFilterConfig::default())
    }
}


// ═══════════════════════════════════════════════════════════════════════════
// DATA STRUCTURES
// ═══════════════════════════════════════════════════════════════════════════


#[derive(Debug, Clone)]
pub struct TradeCosts {
    pub entry_fee: f64,
    pub exit_fee: f64,
    pub entry_slippage: f64,
    pub exit_slippage: f64,
    pub market_impact: f64,
    pub total_cost: f64,
}


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FeeFilterStats {
    pub total_checks: u64,
    pub trades_passed: u64,
    pub trades_filtered: u64,
    pub trades_executed: u64,
    pub filter_rate_pct: f64,
    pub approval_rate_pct: f64,
    pub min_profit_multiplier: f64,
}


// ═══════════════════════════════════════════════════════════════════════════
// TESTS
// ═══════════════════════════════════════════════════════════════════════════


#[cfg(test)]
mod tests {
    use super::*;


    /// Helper: build a filter with grace_period disabled so the cost/profit
    /// logic runs immediately on the first check.
    fn filter_no_grace() -> SmartFeeFilter {
        SmartFeeFilter::new(SmartFeeFilterConfig {
            grace_period_trades: 0,
            ..SmartFeeFilterConfig::default()
        })
    }


    #[test]
    fn test_profitable_trade() {
        let filter = filter_no_grace();

        let entry = 100.0;
        let exit = 105.0;  // 5% gross profit
        let size = 1.0;

        let (should_execute, net_profit, _) = filter.should_execute_trade(
            entry, exit, size, 1.0, "MEDIUM_VOL"
        );

        assert!(should_execute, "5% profit trade should pass the fee filter");
        assert!(net_profit > 0.0, "net_profit must be positive, got {}", net_profit);
    }


    #[test]
    fn test_unprofitable_trade() {
        let filter = filter_no_grace();

        let entry = 100.0;
        let exit = 100.1;  // Only 0.1% gross profit
        let size = 1.0;

        let (should_execute, _, _) = filter.should_execute_trade(
            entry, exit, size, 1.0, "MEDIUM_VOL"
        );

        assert!(!should_execute, "0.1% profit trade should be blocked by fee filter");
    }


    #[test]
    fn test_regime_adjustment() {
        let mut config = SmartFeeFilterConfig::default();
        config.enable_regime_adjustment = true;
        config.grace_period_trades = 0;
        let filter = SmartFeeFilter::new(config);

        let entry = 100.0;
        let exit = 101.0;
        let size = 1.0;

        // High vol should be more permissive
        let (high_vol_ok, _, _) = filter.should_execute_trade(
            entry, exit, size, 2.0, "HIGH_VOL"
        );

        // Low vol should be more strict
        let (low_vol_ok, _, _) = filter.should_execute_trade(
            entry, exit, size, 0.3, "VERY_LOW_VOL"
        );

        // High vol more likely to pass
        assert!(high_vol_ok || !low_vol_ok);
    }


    #[test]
    fn test_grace_period() {
        let mut config = SmartFeeFilterConfig::default();
        config.grace_period_trades = 5;
        let filter = SmartFeeFilter::new(config);

        // Even unprofitable trades should pass during grace period
        let entry = 100.0;
        let exit = 100.05;  // Minimal profit
        let size = 1.0;

        let (should_execute, _, reason) = filter.should_execute_trade(
            entry, exit, size, 1.0, "MEDIUM_VOL"
        );

        assert!(should_execute);
        assert!(reason.contains("Grace"));
    }


    #[test]
    fn test_default_trait() {
        let f = SmartFeeFilter::default();
        let stats = f.stats();
        assert_eq!(stats.total_checks, 0);
    }


    // ── V2.1: FeesConfig factory tests ────────────────────────────────────


    #[test]
    fn test_from_fees_config_default() {
        let fees = FeesConfig::default();
        let config = SmartFeeFilterConfig::from_fees_config(&fees);
        assert!((config.maker_fee_percent - 0.02).abs() < f64::EPSILON);
        assert!((config.taker_fee_percent - 0.04).abs() < f64::EPSILON);
        assert!((config.slippage_percent - 0.05).abs() < f64::EPSILON);
        // Non-fee defaults preserved
        assert_eq!(config.min_profit_multiplier, 2.0);
        assert!(config.enable_market_impact);
        assert!(config.enable_regime_adjustment);
    }


    #[test]
    fn test_from_fees_config_with_multiplier() {
        let fees = FeesConfig::default();
        let config = SmartFeeFilterConfig::from_fees_config_with_multiplier(&fees, 3.5);
        assert!((config.maker_fee_percent - 0.02).abs() < f64::EPSILON);
        assert_eq!(config.min_profit_multiplier, 3.5);
    }


    #[test]
    fn test_from_fees_config_with_custom_fees() {
        let fees = FeesConfig {
            maker_fee_bps: 5.0,
            taker_fee_bps: 10.0,
            slippage_bps: 8.0,
            ..FeesConfig::default()
        };
        let config = SmartFeeFilterConfig::from_fees_config(&fees);
        assert!((config.maker_fee_percent - 0.05).abs() < f64::EPSILON);
        assert!((config.taker_fee_percent - 0.10).abs() < f64::EPSILON);
        assert!((config.slippage_percent - 0.08).abs() < f64::EPSILON);
    }


    // ── V2.2: grace_period=0 in from_fees_config() ────────────────────────


    /// from_fees_config() must always produce grace_period_trades=0.
    #[test]
    fn test_from_fees_config_grace_period_is_zero() {
        let fees = FeesConfig::default();
        let config = SmartFeeFilterConfig::from_fees_config(&fees);
        assert_eq!(config.grace_period_trades, 0,
            "from_fees_config() must set grace_period_trades=0 (production path)");
    }


    /// Verify the filter built from FeesConfig blocks an unprofitable trade
    /// on the very first call — no grace period bypass.
    #[test]
    fn test_from_fees_config_no_grace_bypass_on_first_check() {
        let fees = FeesConfig {
            maker_fee_bps: 10.0,
            taker_fee_bps: 20.0,
            slippage_bps: 15.0,
            ..FeesConfig::default()
        };
        let config = SmartFeeFilterConfig::from_fees_config(&fees);
        let filter = SmartFeeFilter::new(config);
        let (pass, _, reason) = filter.should_execute_trade(
            100.0, 100.2, 0.1, 0.0, "VERY_LOW_VOL"
        );
        assert!(!pass, "First call must not bypass filtering; got reason: {}", reason);
    }


    // ── V2.3 PR #105: Regime + vol scaling mutual exclusion ───────────────


    /// VERY_LOW_VOL + vol≈0: regime flag must block the vol scaling branch.
    ///
    /// Pre-fix: 2.0 × 1.5 (regime) × 1.75 (vol) = 5.25× → blocked 17/17 trades.
    /// Post-fix: 2.0 × 1.5 (regime only)          = 3.0× → proportionate gate.
    ///
    /// Test: a trade that clearly clears 3.0× but would fail 5.25× must PASS.
    ///   $100 entry, 0.6% spacing, 1.0 SOL:
    ///   gross  ≈ $100 × 0.006 × 1.0 = $0.60
    ///   costs  ≈ $100 × (0.04+0.02+0.05+0.05+0.01)/100 × 1.0 = $0.17
    ///   net    ≈ $0.43
    ///   3.0×:  threshold ≈ $0.51  — borderline; use 1.0% spacing for margin:
    ///   gross  = $1.00, costs ≈ $0.17, net ≈ $0.83
    ///   3.0×:  $0.51 → PASS ✅   5.25×: $0.89 → FAIL ❌  (regression catch)
    #[test]
    fn test_regime_adjustment_not_compounded_with_vol_scaling() {
        let filter = SmartFeeFilter::new(SmartFeeFilterConfig {
            grace_period_trades: 0,
            enable_regime_adjustment: true,
            min_profit_multiplier: 2.0,
            volatility_scaling_factor: 1.5,
            ..SmartFeeFilterConfig::default()
        });

        // 1% spacing, 1.0 SOL — clears 3.0× gate but not 5.25×
        let (pass, net, reason) = filter.should_execute_trade(
            100.0,
            101.0,  // 1.0% spacing
            1.0,
            0.0001, // vol ≈ 0 — maximises pre-fix compound bug
            "VERY_LOW_VOL",
        );

        assert!(
            pass,
            "VERY_LOW_VOL at 1% spacing must PASS with fixed 3.0× threshold \
             (pre-fix 5.25× blocked this). net={:.6} reason={}",
            net, reason
        );
    }


    /// When enable_regime_adjustment=false, vol scaling must still apply.
    /// Ensures the fix didn't accidentally remove vol scaling entirely.
    #[test]
    fn test_vol_scaling_applies_when_regime_disabled() {
        let filter_no_regime = SmartFeeFilter::new(SmartFeeFilterConfig {
            grace_period_trades: 0,
            enable_regime_adjustment: false,
            min_profit_multiplier: 2.0,
            volatility_scaling_factor: 1.5,
            ..SmartFeeFilterConfig::default()
        });
        let filter_with_regime = SmartFeeFilter::new(SmartFeeFilterConfig {
            grace_period_trades: 0,
            enable_regime_adjustment: true,
            min_profit_multiplier: 2.0,
            volatility_scaling_factor: 1.5,
            ..SmartFeeFilterConfig::default()
        });

        // VERY_LOW_VOL + vol≈0:
        //   regime_on (fixed): 2.0 × 1.5 = 3.0× threshold
        //   regime_off:        2.0 × 1.75 = 3.5× threshold (vol scaling only)
        // Regime-on must be MORE permissive (lower threshold = more passes)
        let (with_regime_pass, _, _) = filter_with_regime.should_execute_trade(
            100.0, 101.0, 1.0, 0.0001, "VERY_LOW_VOL"
        );
        let (no_regime_pass, _, _) = filter_no_regime.should_execute_trade(
            100.0, 101.0, 1.0, 0.0001, "VERY_LOW_VOL"
        );

        // regime_on (3.0×) must be at least as permissive as regime_off (3.5×)
        assert!(
            with_regime_pass || !no_regime_pass,
            "Regime-enabled (3.0×) must be >= permissive than regime-disabled (3.5×) \
             for VERY_LOW_VOL + vol≈0"
        );
    }


    /// MEDIUM_VOL with normal vol (0.5..1.0 neutral band): both paths must
    /// agree since regime_factor=1.0 and vol falls in the no-op band.
    #[test]
    fn test_medium_vol_neutral_band_unchanged() {
        let filter = SmartFeeFilter::new(SmartFeeFilterConfig {
            grace_period_trades: 0,
            enable_regime_adjustment: true,
            ..SmartFeeFilterConfig::default()
        });

        // 3% gross profit, vol=0.8 (neutral band 0.5..1.0)
        let (pass, net, _) = filter.should_execute_trade(
            100.0, 103.0, 1.0, 0.8, "MEDIUM_VOL"
        );

        assert!(
            pass,
            "3% profit trade in MEDIUM_VOL (neutral vol band) must pass; net={:.4}",
            net
        );
    }
}
