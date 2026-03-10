//! ═════════════════════════════════════════════════════════════════════════
//! ADAPTIVE OPTIMIZER V1.2 - Self-Learning Grid Intelligence
//!
//! FEATURES:
//! [ok] Smart Grid Spacing - Auto-adjust based on performance
//! [ok] Dynamic Position Sizing - Scale orders based on efficiency
//! [ok] Real-time adaptation using EnhancedMetrics
//! [ok] Risk-aware adjustments
//! [ok] Win/Loss streak detection
//! [ok] Warmup guard - requires minimum fills before acting
//! [ok] Skip reason logging - every no-change cycle emits why (PR #92)
//!
//! PHILOSOPHY:
//! "The bot that learns from its wins and losses is the bot that survives."
//!
//! February  9, 2026 - V1.0 initial
//! February 28, 2026 - V1.1: Fix unit bugs + warmup guard
//!   Bug 1: grid_efficiency was stored as 0-100 but thresholds expect 0.0-1.0
//!   Bug 2: MIN/MAX spacing constants were in % units, values are in fractions
//!          Fixed: MIN=0.0001 (0.01%), MAX=0.01 (1.0%) in fraction units.
//!   Bug 3: Optimizer fired at cycle 1 with 0 fills - decisions from noise.
//!          Fixed: min_fills_to_activate guard (default 5).
//! March    10, 2026 - V1.2 PR #92: skip_reason in OptimizationResult
//!   When optimize() returns no changes, reason now carries a human-readable
//!   explanation so callers can always log it at debug.
//! ═════════════════════════════════════════════════════════════════════════

use super::EnhancedMetrics;
use log::{info, debug, warn};

// ═════════════════════════════════════════════════════════════════════════
// CONSTANTS - Tuned for Production
// ═════════════════════════════════════════════════════════════════════════

// Smart Grid Spacing Thresholds
const LOW_DRAWDOWN_THRESHOLD: f64 = 2.0;      // < 2% = doing great, tighten
const MODERATE_DRAWDOWN_THRESHOLD: f64 = 5.0; // 2-5% = normal, maintain
const HIGH_DRAWDOWN_THRESHOLD: f64 = 8.0;     // 5-8% = caution, widen
#[allow(dead_code)] // semantic label; else-branch catches > 8% via SPACING_EMERGENCY_MULTIPLIER
const EMERGENCY_DRAWDOWN_THRESHOLD: f64 = 12.0; // > 8% = emergency, max widen

// Spacing Multipliers
const SPACING_TIGHTEN_MULTIPLIER: f64 = 0.80;   // 20% tighter when winning
const SPACING_NORMAL_MULTIPLIER: f64 = 1.00;    // Baseline
const SPACING_WIDEN_MULTIPLIER: f64 = 1.30;     // 30% wider when losing
const SPACING_EMERGENCY_MULTIPLIER: f64 = 1.80; // 80% wider in emergency

// Spacing Limits - stored as fractions matching current_spacing_percent.
//   e.g. 0.15% config -> 0.0015 stored -> displayed as * 100 = 0.15%
//   MIN = 0.01%  -> 0.0001 fraction
//   MAX = 1.0%   -> 0.01   fraction
const MIN_SPACING_PERCENT: f64 = 0.0001; // 0.01% - ultra tight floor
const MAX_SPACING_PERCENT: f64 = 0.01;   // 1.0%  - wide ceiling

// Dynamic Position Sizing Thresholds
// grid_efficiency is stored as a 0.0-1.0 fraction by EnhancedMetrics
const HIGH_EFFICIENCY_THRESHOLD: f64 = 0.70;  // > 70% efficiency = scale up
const LOW_EFFICIENCY_THRESHOLD: f64  = 0.30;  // < 30% efficiency = scale down

// Position Size Multipliers
const SIZE_HIGH_EFFICIENCY_MULTIPLIER: f64 = 1.30; // 30% bigger orders
const SIZE_NORMAL_MULTIPLIER: f64          = 1.00; // Baseline
const SIZE_LOW_EFFICIENCY_MULTIPLIER: f64  = 0.70; // 30% smaller orders

// Win/Loss Streak Bonuses
const WIN_STREAK_BONUS_MAX: f64    = 1.50; // Up to 50% bigger on win streaks
const LOSS_STREAK_PENALTY_MAX: f64 = 0.60; // Down to 40% smaller on loss streaks
const STREAK_THRESHOLD: usize      = 3;    // 3+ wins/losses triggers adjustment

// Position Size Limits
const MIN_POSITION_SIZE: f64 = 0.05; // 0.05 SOL minimum
const MAX_POSITION_SIZE: f64 = 5.0;  // 5.0 SOL maximum

// ═════════════════════════════════════════════════════════════════════════
// ADAPTIVE OPTIMIZER - Main Struct
// ═════════════════════════════════════════════════════════════════════════

/// Adaptive optimizer that adjusts grid spacing and position sizing
/// based on real-time performance metrics.
/// Does not act until `min_fills_to_activate` fills have been recorded
/// to avoid garbage decisions at startup.
#[derive(Debug, Clone)]
pub struct AdaptiveOptimizer {
    /// Base grid spacing from config (fraction, e.g. 0.0015 = 0.15%)
    pub base_spacing_percent: f64,
    /// Base position size from config (SOL)
    pub base_position_size: f64,
    /// Current adjusted spacing (fraction)
    pub current_spacing_percent: f64,
    /// Current adjusted position size (SOL)
    pub current_position_size: f64,
    /// Number of adjustments made
    pub adjustment_count: u64,
    /// Last adjustment reason
    pub last_reason: String,
    /// Minimum total fills (buys + sells) before optimizer is allowed to act.
    pub min_fills_to_activate: usize,
}

impl AdaptiveOptimizer {
    /// Create new optimizer with base settings from config.
    /// `base_spacing_percent` must be in fraction form (e.g. 0.0015 for 0.15%).
    pub fn new(base_spacing_percent: f64, base_position_size: f64) -> Self {
        info!("[OPT] Initializing Adaptive Optimizer");
        info!("[OPT] Base Spacing: {:.4}% ({:.6} fraction)",
              base_spacing_percent * 100.0, base_spacing_percent);
        info!("[OPT] Base Position: {} SOL", base_position_size);
        info!("[OPT] Warmup: {} fills required before optimization", 5);

        Self {
            base_spacing_percent,
            base_position_size,
            current_spacing_percent: base_spacing_percent,
            current_position_size:   base_position_size,
            adjustment_count:        0,
            last_reason:             "Initialized".to_string(),
            min_fills_to_activate:   5,
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // SMART GRID SPACING
    // ═════════════════════════════════════════════════════════════════════

    /// Calculate optimal grid spacing based on current drawdown.
    /// Returns a fraction (same units as `current_spacing_percent`).
    pub fn calculate_optimal_spacing(&self, metrics: &EnhancedMetrics) -> f64 {
        let drawdown = metrics.max_drawdown;

        let multiplier = if drawdown < LOW_DRAWDOWN_THRESHOLD {
            debug!("[OPT] Low drawdown ({:.2}%) - tightening grid", drawdown);
            SPACING_TIGHTEN_MULTIPLIER
        } else if drawdown < MODERATE_DRAWDOWN_THRESHOLD {
            debug!("[OPT] Moderate drawdown ({:.2}%) - maintaining grid", drawdown);
            SPACING_NORMAL_MULTIPLIER
        } else if drawdown < HIGH_DRAWDOWN_THRESHOLD {
            debug!("[OPT] High drawdown ({:.2}%) - widening grid", drawdown);
            SPACING_WIDEN_MULTIPLIER
        } else {
            warn!("[OPT] EMERGENCY drawdown ({:.2}%) - max widening!", drawdown);
            SPACING_EMERGENCY_MULTIPLIER
        };

        let new_spacing = self.base_spacing_percent * multiplier;
        new_spacing.clamp(MIN_SPACING_PERCENT, MAX_SPACING_PERCENT)
    }

    /// Update grid spacing based on metrics. Returns true if spacing changed.
    pub fn update_spacing(&mut self, metrics: &EnhancedMetrics) -> bool {
        let old_spacing = self.current_spacing_percent;
        let new_spacing = self.calculate_optimal_spacing(metrics);

        let change_pct = ((new_spacing - old_spacing).abs() / old_spacing) * 100.0;

        if change_pct > 5.0 {
            self.current_spacing_percent = new_spacing;
            self.adjustment_count += 1;
            self.last_reason = format!(
                "Drawdown {:.2}%: {:.4}% -> {:.4}%",
                metrics.max_drawdown,
                old_spacing * 100.0,
                new_spacing * 100.0
            );
            info!("[OPT] SPACING ADJUSTED: {}", self.last_reason);
            true
        } else {
            debug!("[OPT] Spacing change too small ({:.1}%), keeping current", change_pct);
            false
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // DYNAMIC POSITION SIZING
    // ═════════════════════════════════════════════════════════════════════

    /// Calculate optimal position size based on grid efficiency.
    /// `metrics.grid_efficiency` must be a 0.0-1.0 fraction.
    pub fn calculate_optimal_position_size(&self, metrics: &EnhancedMetrics) -> f64 {
        let efficiency = metrics.grid_efficiency;

        let efficiency_multiplier = if efficiency > HIGH_EFFICIENCY_THRESHOLD {
            debug!("[OPT] High efficiency ({:.1}%) - scaling up", efficiency * 100.0);
            SIZE_HIGH_EFFICIENCY_MULTIPLIER
        } else if efficiency < LOW_EFFICIENCY_THRESHOLD {
            debug!("[OPT] Low efficiency ({:.1}%) - scaling down", efficiency * 100.0);
            SIZE_LOW_EFFICIENCY_MULTIPLIER
        } else {
            SIZE_NORMAL_MULTIPLIER
        };

        let streak_multiplier = self.calculate_streak_multiplier(metrics);
        let new_size = self.base_position_size * efficiency_multiplier * streak_multiplier;
        new_size.clamp(MIN_POSITION_SIZE, MAX_POSITION_SIZE)
    }

    fn calculate_streak_multiplier(&self, metrics: &EnhancedMetrics) -> f64 {
        let total_trades = metrics.profitable_trades + metrics.unprofitable_trades;
        if total_trades < STREAK_THRESHOLD {
            return 1.0;
        }

        let win_rate = metrics.profitable_trades as f64 / total_trades as f64;

        if win_rate > 0.70 {
            let bonus = 1.0 + ((win_rate - 0.70) * 1.67);
            debug!("[OPT] Win streak ({:.0}%) - bonus {:.2}x", win_rate * 100.0, bonus);
            bonus.min(WIN_STREAK_BONUS_MAX)
        } else if win_rate < 0.40 {
            let penalty = 0.6 + (win_rate * 1.0);
            debug!("[OPT] Loss streak ({:.0}%) - penalty {:.2}x", win_rate * 100.0, penalty);
            penalty.max(LOSS_STREAK_PENALTY_MAX)
        } else {
            1.0
        }
    }

    /// Update position size based on metrics. Returns true if size changed.
    pub fn update_position_size(&mut self, metrics: &EnhancedMetrics) -> bool {
        let old_size = self.current_position_size;
        let new_size = self.calculate_optimal_position_size(metrics);

        let change_pct = ((new_size - old_size).abs() / old_size) * 100.0;

        if change_pct > 10.0 {
            self.current_position_size = new_size;
            self.adjustment_count += 1;
            self.last_reason = format!(
                "Efficiency {:.1}%: {:.3} -> {:.3} SOL",
                metrics.grid_efficiency * 100.0, old_size, new_size
            );
            info!("[OPT] POSITION SIZE ADJUSTED: {}", self.last_reason);
            true
        } else {
            debug!("[OPT] Size change too small ({:.1}%), keeping current", change_pct);
            false
        }
    }

    // ═════════════════════════════════════════════════════════════════════
    // MAIN OPTIMIZATION LOOP
    // ═════════════════════════════════════════════════════════════════════

    /// Run full optimization cycle - adjust both spacing and position size.
    /// Returns early (no changes) until `min_fills_to_activate` fills have
    /// been recorded, preventing noise-driven decisions at startup.
    ///
    /// PR #92: `result.reason` is always populated:
    /// - Warming up  -> "Warming up (n/m fills)"
    /// - Threshold not met -> "threshold not met (drawdown=X% eff=Y%)"
    /// - Changes applied -> last_reason from spacing/size update
    pub fn optimize(&mut self, metrics: &EnhancedMetrics) -> OptimizationResult {
        debug!("[OPT] Running optimization cycle #{}", self.adjustment_count + 1);

        let total_fills = metrics.total_buys + metrics.total_sells;
        if total_fills < self.min_fills_to_activate {
            debug!("[OPT] Warming up ({}/{} fills needed)", total_fills, self.min_fills_to_activate);
            return OptimizationResult {
                spacing_adjusted:  false,
                size_adjusted:     false,
                new_spacing:       self.current_spacing_percent,
                new_position_size: self.current_position_size,
                reason: format!("Warming up ({}/{})", total_fills, self.min_fills_to_activate),
            };
        }

        let spacing_changed = self.update_spacing(metrics);
        let size_changed    = self.update_position_size(metrics);

        if spacing_changed || size_changed {
            info!("[OPT] Applied: spacing={:.4}%, size={:.3} SOL",
                  self.current_spacing_percent * 100.0, self.current_position_size);
        }

        // PR #92 P1: Always populate reason - callers log it unconditionally.
        let reason = if spacing_changed || size_changed {
            self.last_reason.clone()
        } else {
            format!(
                "threshold not met (drawdown={:.2}% eff={:.1}%)",
                metrics.max_drawdown,
                metrics.grid_efficiency * 100.0
            )
        };

        OptimizationResult {
            spacing_adjusted:  spacing_changed,
            size_adjusted:     size_changed,
            new_spacing:       self.current_spacing_percent,
            new_position_size: self.current_position_size,
            reason,
        }
    }

    pub fn display(&self) {
        println!("\n[OPT] ADAPTIVE OPTIMIZER STATUS:");
        println!("   Adjustments Made:   {}", self.adjustment_count);
        println!("   Current Spacing:    {:.4}% (base: {:.4}%)",
                 self.current_spacing_percent * 100.0,
                 self.base_spacing_percent    * 100.0);
        println!("   Current Size:       {:.3} SOL (base: {:.3} SOL)",
                 self.current_position_size, self.base_position_size);
        if !self.last_reason.is_empty() && self.last_reason != "Initialized" {
            println!("   Last Adjustment:    {}", self.last_reason);
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// RESULT TYPE
// ═════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub spacing_adjusted:  bool,
    pub size_adjusted:     bool,
    pub new_spacing:       f64,
    pub new_position_size: f64,
    pub reason:            String,
}

impl OptimizationResult {
    pub fn any_changes(&self) -> bool {
        self.spacing_adjusted || self.size_adjusted
    }
}

// ═════════════════════════════════════════════════════════════════════════
// TESTS
// ═════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_optimizer_creation() {
        let opt = AdaptiveOptimizer::new(0.0015, 0.1);
        assert_eq!(opt.base_spacing_percent,    0.0015);
        assert_eq!(opt.base_position_size,      0.1);
        assert_eq!(opt.current_spacing_percent, 0.0015);
        assert_eq!(opt.current_position_size,   0.1);
        assert_eq!(opt.min_fills_to_activate,   5);
    }

    #[test]
    fn test_spacing_tightens_on_low_drawdown() {
        let opt = AdaptiveOptimizer::new(0.0015, 0.1);
        let mut m = EnhancedMetrics::new();
        m.max_drawdown = 1.0;
        assert!(opt.calculate_optimal_spacing(&m) < opt.base_spacing_percent);
    }

    #[test]
    fn test_spacing_widens_on_high_drawdown() {
        let opt = AdaptiveOptimizer::new(0.0015, 0.1);
        let mut m = EnhancedMetrics::new();
        m.max_drawdown = 10.0;
        assert!(opt.calculate_optimal_spacing(&m) > opt.base_spacing_percent);
    }

    #[test]
    fn test_position_size_scales_with_efficiency() {
        let opt = AdaptiveOptimizer::new(0.0015, 0.1);
        let mut m = EnhancedMetrics::new();
        m.grid_efficiency = 0.80;
        let high = opt.calculate_optimal_position_size(&m);
        m.grid_efficiency = 0.20;
        let low  = opt.calculate_optimal_position_size(&m);
        assert!(high > low);
    }

    #[test]
    fn test_limits_enforced() {
        let opt = AdaptiveOptimizer::new(0.0015, 0.01);
        let mut m = EnhancedMetrics::new();
        m.max_drawdown = 50.0;
        let spacing = opt.calculate_optimal_spacing(&m);
        assert!(spacing <= MAX_SPACING_PERCENT);
        assert!(spacing >= MIN_SPACING_PERCENT);
        let size = opt.calculate_optimal_position_size(&m);
        assert!(size <= MAX_POSITION_SIZE);
        assert!(size >= MIN_POSITION_SIZE);
    }

    #[test]
    fn test_warmup_guard_blocks_early_optimization() {
        let mut opt = AdaptiveOptimizer::new(0.0015, 0.1);
        let mut m   = EnhancedMetrics::new();
        m.max_drawdown    = 10.0;
        m.grid_efficiency = 0.80;
        let result = opt.optimize(&m);
        assert!(!result.spacing_adjusted);
        assert!(!result.size_adjusted);
        assert!(result.reason.contains("Warming up"));
        assert_eq!(opt.current_spacing_percent, 0.0015);
    }

    #[test]
    fn test_warmup_guard_releases_after_fills() {
        let mut opt = AdaptiveOptimizer::new(0.0015, 0.1);
        let mut m   = EnhancedMetrics::new();
        m.max_drawdown = 10.0;
        m.total_buys   = 3;
        m.total_sells  = 2;
        let result = opt.optimize(&m);
        assert!(!result.reason.contains("Warming up"));
    }

    /// PR #92 P1: optimize() must always return a non-empty reason when no
    /// adjustment occurs.
    ///
    /// max_drawdown = 3.5  -> moderate band (2.0-5.0) -> SPACING_NORMAL_MULTIPLIER 1.00
    ///   -> new_spacing = 0.0015 * 1.00 = 0.0015 -> 0% change < 5% gate -> NOT adjusted
    /// grid_efficiency = 0.50 -> normal band (0.30-0.70) -> SIZE_NORMAL_MULTIPLIER 1.00
    ///   -> new_size = 0.1 * 1.00 = 0.1 -> 0% change < 10% gate -> NOT adjusted
    /// Result: reason = "threshold not met (drawdown=3.50% eff=50.0%)"
    #[test]
    fn test_optimize_no_change_emits_skip_reason() {
        let mut opt = AdaptiveOptimizer::new(0.0015, 0.1);
        let mut m   = EnhancedMetrics::new();
        m.total_buys      = 3;
        m.total_sells     = 2;
        m.max_drawdown    = 3.5;  // moderate band -> 1.00x multiplier -> 0% spacing change
        m.grid_efficiency = 0.50; // normal band   -> 1.00x multiplier -> 0% size change
        let result = opt.optimize(&m);
        assert!(!result.spacing_adjusted, "spacing must not be adjusted with 0% change");
        assert!(!result.size_adjusted,    "size must not be adjusted with 0% change");
        assert!(!result.reason.is_empty(), "reason must never be empty");
        assert!(
            result.reason.contains("threshold")
                || result.reason.contains("Warming")
                || result.reason.contains("drawdown"),
            "reason '{}' must contain diagnostic context",
            result.reason
        );
    }

    /// PR #92 P1: reason contains drawdown + efficiency values when threshold not met.
    #[test]
    fn test_optimize_skip_reason_contains_metrics() {
        let mut opt = AdaptiveOptimizer::new(0.0015, 0.1);
        let mut m   = EnhancedMetrics::new();
        m.total_buys      = 3;
        m.total_sells     = 2;
        m.max_drawdown    = 3.5;  // moderate band - SPACING_NORMAL_MULTIPLIER -> 0% change
        m.grid_efficiency = 0.55; // normal band   - SIZE_NORMAL_MULTIPLIER   -> 0% change
        let result = opt.optimize(&m);
        if !result.any_changes() {
            assert!(
                result.reason.contains("drawdown"),
                "skip reason '{}' must include drawdown context", result.reason
            );
            assert!(
                result.reason.contains("eff"),
                "skip reason '{}' must include efficiency context", result.reason
            );
        }
    }
}
