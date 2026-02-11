//! ═════════════════════════════════════════════════════════════════════════
//! 🧠 ADAPTIVE OPTIMIZER V2.0 - Self-Learning Grid Intelligence (CONFIG-DRIVEN!)
//!
//! V2.0 ENHANCEMENTS - Fully Configurable:
//! ✅ Config-driven thresholds and multipliers
//! ✅ Backward compatible with V1.0
//! ✅ Validation and safety limits from config
//! ✅ Dynamic spacing AND position sizing
//! ✅ Streak detection and bonuses
//!
//! PHILOSOPHY:
//! "The bot that learns from config AND performance is the bot that dominates."
//!
//! February 11, 2026 - V2.0 CONFIG INTEGRATION COMPLETE! 🔥
//! ═════════════════════════════════════════════════════════════════════════

use super::EnhancedMetrics;
use log::{info, debug, warn};

// ═════════════════════════════════════════════════════════════════════════
// V1.0 DEFAULTS - For backward compatibility
// ═════════════════════════════════════════════════════════════════════════

const V1_LOW_DRAWDOWN_THRESHOLD: f64 = 2.0;
const V1_MODERATE_DRAWDOWN_THRESHOLD: f64 = 5.0;
const V1_HIGH_DRAWDOWN_THRESHOLD: f64 = 8.0;
const V1_EMERGENCY_DRAWDOWN_THRESHOLD: f64 = 12.0;

const V1_SPACING_TIGHTEN_MULTIPLIER: f64 = 0.80;
const V1_SPACING_NORMAL_MULTIPLIER: f64 = 1.00;
const V1_SPACING_WIDEN_MULTIPLIER: f64 = 1.30;
const V1_SPACING_EMERGENCY_MULTIPLIER: f64 = 1.80;

const V1_MIN_SPACING_PERCENT: f64 = 0.01;
const V1_MAX_SPACING_PERCENT: f64 = 1.00;

const V1_HIGH_EFFICIENCY_THRESHOLD: f64 = 0.70;
const V1_LOW_EFFICIENCY_THRESHOLD: f64 = 0.30;

const V1_SIZE_HIGH_EFFICIENCY_MULTIPLIER: f64 = 1.30;
const V1_SIZE_NORMAL_MULTIPLIER: f64 = 1.00;
const V1_SIZE_LOW_EFFICIENCY_MULTIPLIER: f64 = 0.70;

const V1_WIN_STREAK_BONUS_MAX: f64 = 1.50;
const V1_LOSS_STREAK_PENALTY_MAX: f64 = 0.60;
const V1_STREAK_THRESHOLD: usize = 3;

const V1_MIN_POSITION_SIZE: f64 = 0.05;
const V1_MAX_POSITION_SIZE: f64 = 5.0;

// ═════════════════════════════════════════════════════════════════════════
// V2.0 CONFIG STRUCT - Injected at runtime
// ═════════════════════════════════════════════════════════════════════════

/// Runtime configuration for adaptive optimizer
/// Can be built from TOML config or use V1.0 defaults
#[derive(Debug, Clone)]
pub struct OptimizerConfig {
    // Drawdown thresholds
    pub low_drawdown_threshold: f64,
    pub moderate_drawdown_threshold: f64,
    pub high_drawdown_threshold: f64,
    pub emergency_drawdown_threshold: f64,
    
    // Spacing multipliers
    pub spacing_tighten_multiplier: f64,
    pub spacing_normal_multiplier: f64,
    pub spacing_widen_multiplier: f64,
    pub spacing_emergency_multiplier: f64,
    
    // Spacing limits
    pub min_spacing_percent: f64,
    pub max_spacing_percent: f64,
    
    // Efficiency thresholds
    pub high_efficiency_threshold: f64,
    pub low_efficiency_threshold: f64,
    
    // Sizing multipliers
    pub size_high_efficiency_multiplier: f64,
    pub size_low_efficiency_multiplier: f64,
    
    // Streak parameters
    pub win_streak_bonus_max: f64,
    pub loss_streak_penalty_max: f64,
    pub streak_threshold: usize,
    
    // Position limits
    pub min_position_size: f64,
    pub max_position_size: f64,
}

impl Default for OptimizerConfig {
    fn default() -> Self {
        Self {
            low_drawdown_threshold: V1_LOW_DRAWDOWN_THRESHOLD,
            moderate_drawdown_threshold: V1_MODERATE_DRAWDOWN_THRESHOLD,
            high_drawdown_threshold: V1_HIGH_DRAWDOWN_THRESHOLD,
            emergency_drawdown_threshold: V1_EMERGENCY_DRAWDOWN_THRESHOLD,
            spacing_tighten_multiplier: V1_SPACING_TIGHTEN_MULTIPLIER,
            spacing_normal_multiplier: V1_SPACING_NORMAL_MULTIPLIER,
            spacing_widen_multiplier: V1_SPACING_WIDEN_MULTIPLIER,
            spacing_emergency_multiplier: V1_SPACING_EMERGENCY_MULTIPLIER,
            min_spacing_percent: V1_MIN_SPACING_PERCENT,
            max_spacing_percent: V1_MAX_SPACING_PERCENT,
            high_efficiency_threshold: V1_HIGH_EFFICIENCY_THRESHOLD,
            low_efficiency_threshold: V1_LOW_EFFICIENCY_THRESHOLD,
            size_high_efficiency_multiplier: V1_SIZE_HIGH_EFFICIENCY_MULTIPLIER,
            size_low_efficiency_multiplier: V1_SIZE_LOW_EFFICIENCY_MULTIPLIER,
            win_streak_bonus_max: V1_WIN_STREAK_BONUS_MAX,
            loss_streak_penalty_max: V1_LOSS_STREAK_PENALTY_MAX,
            streak_threshold: V1_STREAK_THRESHOLD,
            min_position_size: V1_MIN_POSITION_SIZE,
            max_position_size: V1_MAX_POSITION_SIZE,
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// ADAPTIVE OPTIMIZER - Now Config-Driven!
// ═════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone)]
pub struct AdaptiveOptimizer {
    pub base_spacing_percent: f64,
    pub base_position_size: f64,
    pub current_spacing_percent: f64,
    pub current_position_size: f64,
    pub adjustment_count: u64,
    pub last_reason: String,
    
    // 🔥 NEW: Runtime config
    config: OptimizerConfig,
}

impl AdaptiveOptimizer {
    /// V1.0 Constructor - Backward compatible with default constants
    pub fn new(base_spacing_percent: f64, base_position_size: f64) -> Self {
        Self::with_config(base_spacing_percent, base_position_size, OptimizerConfig::default())
    }
    
    /// V2.0 Constructor - Config-driven for full control
    pub fn with_config(
        base_spacing_percent: f64,
        base_position_size: f64,
        config: OptimizerConfig,
    ) -> Self {
        info!("🧠 Initializing Adaptive Optimizer V2.0 (CONFIG-DRIVEN)");
        info!("   Base Spacing: {:.3}%", base_spacing_percent * 100.0);
        info!("   Base Position: {} SOL", base_position_size);
        info!("   Config Limits:");
        info!("     Spacing: {:.3}% - {:.3}%", 
              config.min_spacing_percent * 100.0,
              config.max_spacing_percent * 100.0);
        info!("     Position: {:.3} - {:.3} SOL",
              config.min_position_size,
              config.max_position_size);
        
        Self {
            base_spacing_percent,
            base_position_size,
            current_spacing_percent: base_spacing_percent,
            current_position_size: base_position_size,
            adjustment_count: 0,
            last_reason: "Initialized".to_string(),
            config,
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // 🧠 SMART GRID SPACING (Now uses config thresholds!)
    // ═══════════════════════════════════════════════════════════════════
    
    pub fn calculate_optimal_spacing(&self, metrics: &EnhancedMetrics) -> f64 {
        let drawdown = metrics.max_drawdown;
        
        let multiplier = if drawdown < self.config.low_drawdown_threshold {
            debug!("🎯 Low drawdown ({:.2}%) - tightening grid", drawdown);
            self.config.spacing_tighten_multiplier
        } else if drawdown < self.config.moderate_drawdown_threshold {
            debug!("⚖️ Moderate drawdown ({:.2}%) - maintaining grid", drawdown);
            self.config.spacing_normal_multiplier
        } else if drawdown < self.config.high_drawdown_threshold {
            debug!("⚠️ High drawdown ({:.2}%) - widening grid", drawdown);
            self.config.spacing_widen_multiplier
        } else {
            warn!("🚨 EMERGENCY drawdown ({:.2}%) - max widening!", drawdown);
            self.config.spacing_emergency_multiplier
        };
        
        let new_spacing = self.base_spacing_percent * multiplier;
        
        // Clamp to config limits
        new_spacing.clamp(
            self.config.min_spacing_percent,
            self.config.max_spacing_percent
        )
    }
    
    pub fn update_spacing(&mut self, metrics: &EnhancedMetrics) -> bool {
        let old_spacing = self.current_spacing_percent;
        let new_spacing = self.calculate_optimal_spacing(metrics);
        
        let change_pct = ((new_spacing - old_spacing).abs() / old_spacing) * 100.0;
        
        if change_pct > 5.0 {
            self.current_spacing_percent = new_spacing;
            self.adjustment_count += 1;
            
            let drawdown = metrics.max_drawdown;
            self.last_reason = format!(
                "Drawdown {:.2}%: {:.3}% → {:.3}%",
                drawdown, old_spacing * 100.0, new_spacing * 100.0
            );
            
            info!("🧠 SPACING ADJUSTED: {}", self.last_reason);
            true
        } else {
            debug!("Spacing change too small ({:.1}%), keeping current", change_pct);
            false
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // ⚡ DYNAMIC POSITION SIZING (Now uses config thresholds!)
    // ═══════════════════════════════════════════════════════════════════
    
    pub fn calculate_optimal_position_size(&self, metrics: &EnhancedMetrics) -> f64 {
        let efficiency = metrics.grid_efficiency;
        
        let efficiency_multiplier = if efficiency > self.config.high_efficiency_threshold {
            debug!("📈 High efficiency ({:.1}%) - scaling up orders", efficiency * 100.0);
            self.config.size_high_efficiency_multiplier
        } else if efficiency < self.config.low_efficiency_threshold {
            debug!("📉 Low efficiency ({:.1}%) - scaling down orders", efficiency * 100.0);
            self.config.size_low_efficiency_multiplier
        } else {
            V1_SIZE_NORMAL_MULTIPLIER
        };
        
        let streak_multiplier = self.calculate_streak_multiplier(metrics);
        let total_multiplier = efficiency_multiplier * streak_multiplier;
        let new_size = self.base_position_size * total_multiplier;
        
        // Clamp to config limits
        new_size.clamp(
            self.config.min_position_size,
            self.config.max_position_size
        )
    }
    
    fn calculate_streak_multiplier(&self, metrics: &EnhancedMetrics) -> f64 {
        let total_trades = metrics.profitable_trades + metrics.unprofitable_trades;
        
        if total_trades < self.config.streak_threshold {
            return 1.0;
        }
        
        let win_rate = if total_trades > 0 {
            metrics.profitable_trades as f64 / total_trades as f64
        } else {
            0.5
        };
        
        if win_rate > 0.70 {
            let bonus = 1.0 + ((win_rate - 0.70) * 1.67);
            debug!("✨ Win streak detected ({:.0}%) - bonus {:.2}x", win_rate * 100.0, bonus);
            bonus.min(self.config.win_streak_bonus_max)
        } else if win_rate < 0.40 {
            let penalty = 0.6 + (win_rate * 1.0);
            debug!("🚫 Loss streak detected ({:.0}%) - penalty {:.2}x", win_rate * 100.0, penalty);
            penalty.max(self.config.loss_streak_penalty_max)
        } else {
            1.0
        }
    }
    
    pub fn update_position_size(&mut self, metrics: &EnhancedMetrics) -> bool {
        let old_size = self.current_position_size;
        let new_size = self.calculate_optimal_position_size(metrics);
        
        let change_pct = ((new_size - old_size).abs() / old_size) * 100.0;
        
        if change_pct > 10.0 {
            self.current_position_size = new_size;
            self.adjustment_count += 1;
            
            let efficiency = metrics.grid_efficiency;
            self.last_reason = format!(
                "Efficiency {:.1}%: {:.3} → {:.3} SOL",
                efficiency * 100.0, old_size, new_size
            );
            
            info!("⚡ POSITION SIZE ADJUSTED: {}", self.last_reason);
            true
        } else {
            debug!("Size change too small ({:.1}%), keeping current", change_pct);
            false
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // 📊 MAIN OPTIMIZATION LOOP
    // ═══════════════════════════════════════════════════════════════════
    
    pub fn optimize(&mut self, metrics: &EnhancedMetrics) -> OptimizationResult {
        debug!("🧠 Running optimization cycle #{}", self.adjustment_count + 1);
        
        let spacing_changed = self.update_spacing(metrics);
        let size_changed = self.update_position_size(metrics);
        
        if spacing_changed || size_changed {
            info!("✅ Optimization applied: spacing={:.3}%, size={:.3} SOL",
                  self.current_spacing_percent * 100.0, self.current_position_size);
        }
        
        OptimizationResult {
            spacing_adjusted: spacing_changed,
            size_adjusted: size_changed,
            new_spacing: self.current_spacing_percent,
            new_position_size: self.current_position_size,
            reason: self.last_reason.clone(),
        }
    }
    
    pub fn display(&self) {
        println!("\n🧠 ADAPTIVE OPTIMIZER V2.0 STATUS:");
        println!("   Adjustments Made:   {}", self.adjustment_count);
        println!("   Current Spacing:    {:.3}% (base: {:.3}%)",
                 self.current_spacing_percent * 100.0, self.base_spacing_percent * 100.0);
        println!("   Current Size:       {:.3} SOL (base: {:.3} SOL)",
                 self.current_position_size, self.base_position_size);
        println!("   Config Limits:");
        println!("     Spacing Range:    {:.3}% - {:.3}%",
                 self.config.min_spacing_percent * 100.0,
                 self.config.max_spacing_percent * 100.0);
        println!("     Position Range:   {:.3} - {:.3} SOL",
                 self.config.min_position_size,
                 self.config.max_position_size);
        
        if !self.last_reason.is_empty() && self.last_reason != "Initialized" {
            println!("   Last Adjustment:    {}", self.last_reason);
        }
    }
}

#[derive(Debug, Clone)]
pub struct OptimizationResult {
    pub spacing_adjusted: bool,
    pub size_adjusted: bool,
    pub new_spacing: f64,
    pub new_position_size: f64,
    pub reason: String,
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
    fn test_v1_backward_compatibility() {
        let optimizer = AdaptiveOptimizer::new(0.15, 0.1);
        assert_eq!(optimizer.base_spacing_percent, 0.15);
        assert_eq!(optimizer.base_position_size, 0.1);
    }

    #[test]
    fn test_v2_config_constructor() {
        let mut config = OptimizerConfig::default();
        config.min_spacing_percent = 0.10;
        config.max_spacing_percent = 0.50;
        
        let optimizer = AdaptiveOptimizer::with_config(0.20, 0.15, config);
        assert_eq!(optimizer.config.min_spacing_percent, 0.10);
        assert_eq!(optimizer.config.max_spacing_percent, 0.50);
    }

    #[test]
    fn test_spacing_respects_config_limits() {
        let mut config = OptimizerConfig::default();
        config.min_spacing_percent = 0.15;
        config.max_spacing_percent = 0.40;
        
        let optimizer = AdaptiveOptimizer::with_config(0.30, 0.1, config);
        let mut metrics = EnhancedMetrics::new();
        metrics.max_drawdown = 15.0; // Extreme drawdown
        
        let spacing = optimizer.calculate_optimal_spacing(&metrics);
        assert!(spacing <= 0.40); // Should not exceed max
        assert!(spacing >= 0.15); // Should not go below min
    }
}
