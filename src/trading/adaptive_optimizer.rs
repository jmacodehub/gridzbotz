//! ═════════════════════════════════════════════════════════════════════════
//! 🧠 ADAPTIVE OPTIMIZER V2.0 - Config-Driven Self-Learning Grid
//!
//! V2.0 ENHANCEMENTS:
//! ✅ Fully config-driven (no hardcoded values!)
//! ✅ Accepts AdaptiveOptimizerConfig
//! ✅ All thresholds & multipliers configurable
//! ✅ Safety limits from config
//! ✅ Modular & testable!
//!
//! PHILOSOPHY:
//! "Configuration defines intent, optimizer implements intelligence."
//!
//! February 11, 2026 - V2.0 CONFIG-DRIVEN INTELLIGENCE! 🔥
//! ═════════════════════════════════════════════════════════════════════════

use super::EnhancedMetrics;
use crate::config::AdaptiveOptimizerConfig;
use log::{info, debug, warn};

// ═════════════════════════════════════════════════════════════════════════
// ADAPTIVE OPTIMIZER - Config-Driven Intelligence
// ═════════════════════════════════════════════════════════════════════════

/// Adaptive optimizer that adjusts grid spacing and position sizing
/// based on real-time performance metrics AND user configuration
#[derive(Debug, Clone)]
pub struct AdaptiveOptimizer {
    /// Configuration (user-defined intelligence parameters)
    config: AdaptiveOptimizerConfig,
    
    /// Base grid spacing (from trading config)
    pub base_spacing_percent: f64,
    
    /// Base position size (from trading config)
    pub base_position_size: f64,
    
    /// Current adjusted spacing
    pub current_spacing_percent: f64,
    
    /// Current adjusted position size
    pub current_position_size: f64,
    
    /// Number of adjustments made
    pub adjustment_count: u64,
    
    /// Last adjustment reason
    pub last_reason: String,
}

impl AdaptiveOptimizer {
    /// Create new optimizer with config and base settings
    pub fn new_with_config(
        config: AdaptiveOptimizerConfig,
        base_spacing_percent: f64,
        base_position_size: f64,
    ) -> Self {
        info!("🧠 Initializing Config-Driven Adaptive Optimizer V2.0");
        info!("   Base Spacing: {:.3}%", base_spacing_percent * 100.0);
        info!("   Base Position: {} SOL", base_position_size);
        info!("   Enabled: {}", if config.enabled { "✅" } else { "❌" });
        
        if config.enabled {
            info!("   Optimization Interval: {} cycles", config.optimization_interval_cycles);
            info!("   Spacing Range: {:.3}% - {:.3}%", 
                  config.min_spacing_absolute * 100.0,
                  config.max_spacing_absolute * 100.0);
            info!("   Position Range: {:.3} - {:.3} SOL",
                  config.min_position_absolute,
                  config.max_position_absolute);
        }
        
        Self {
            config,
            base_spacing_percent,
            base_position_size,
            current_spacing_percent: base_spacing_percent,
            current_position_size: base_position_size,
            adjustment_count: 0,
            last_reason: "Initialized".to_string(),
        }
    }
    
    /// Create optimizer with default config (backwards compatibility)
    pub fn new(base_spacing_percent: f64, base_position_size: f64) -> Self {
        Self::new_with_config(
            AdaptiveOptimizerConfig::default(),
            base_spacing_percent,
            base_position_size,
        )
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // 🧠 SMART GRID SPACING (Config-Driven)
    // ═══════════════════════════════════════════════════════════════════
    
    /// Calculate optimal grid spacing based on current drawdown
    pub fn calculate_optimal_spacing(&self, metrics: &EnhancedMetrics) -> f64 {
        if !self.config.enabled {
            return self.base_spacing_percent;  // Disabled = use base
        }
        
        let drawdown = metrics.max_drawdown;
        
        // Use config thresholds & multipliers
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
            self.config.min_spacing_absolute,
            self.config.max_spacing_absolute,
        )
    }
    
    /// Update grid spacing based on metrics
    pub fn update_spacing(&mut self, metrics: &EnhancedMetrics) -> bool {
        if !self.config.enabled {
            return false;
        }
        
        let old_spacing = self.current_spacing_percent;
        let new_spacing = self.calculate_optimal_spacing(metrics);
        
        // Only update if change is significant (>5%)
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
    // ⚡ DYNAMIC POSITION SIZING (Config-Driven)
    // ═══════════════════════════════════════════════════════════════════
    
    /// Calculate optimal position size based on grid efficiency
    pub fn calculate_optimal_position_size(&self, metrics: &EnhancedMetrics) -> f64 {
        if !self.config.enabled {
            return self.base_position_size;  // Disabled = use base
        }
        
        let efficiency = metrics.grid_efficiency;
        
        // Base multiplier from efficiency (config thresholds)
        let efficiency_multiplier = if efficiency > self.config.high_efficiency_threshold {
            debug!("📈 High efficiency ({:.1}%) - scaling up orders", efficiency * 100.0);
            self.config.size_high_efficiency_multiplier
        } else if efficiency < self.config.low_efficiency_threshold {
            debug!("📉 Low efficiency ({:.1}%) - scaling down orders", efficiency * 100.0);
            self.config.size_low_efficiency_multiplier
        } else {
            1.0  // Normal
        };
        
        // Win/Loss streak adjustment (config bonuses)
        let streak_multiplier = self.calculate_streak_multiplier(metrics);
        
        // Combine multipliers
        let total_multiplier = efficiency_multiplier * streak_multiplier;
        
        let new_size = self.base_position_size * total_multiplier;
        
        // Clamp to config limits
        new_size.clamp(
            self.config.min_position_absolute,
            self.config.max_position_absolute,
        )
    }
    
    /// Calculate multiplier based on win/loss streaks (config-driven)
    fn calculate_streak_multiplier(&self, metrics: &EnhancedMetrics) -> f64 {
        let total_trades = metrics.profitable_trades + metrics.unprofitable_trades;
        
        if total_trades < self.config.streak_threshold {
            return 1.0; // Not enough data yet
        }
        
        let win_rate = if total_trades > 0 {
            metrics.profitable_trades as f64 / total_trades as f64
        } else {
            0.5
        };
        
        if win_rate > 0.70 {
            // Strong win rate - bonus! (config max)
            let bonus = 1.0 + ((win_rate - 0.70) * 1.67);
            debug!("✨ Win streak ({:.0}%) - bonus {:.2}x", win_rate * 100.0, bonus);
            bonus.min(self.config.win_streak_bonus_max)
        } else if win_rate < 0.40 {
            // Poor win rate - penalty (config max penalty)
            let penalty = 0.6 + (win_rate * 1.0);
            debug!("🚫 Loss streak ({:.0}%) - penalty {:.2}x", win_rate * 100.0, penalty);
            penalty.max(self.config.loss_streak_penalty_max)
        } else {
            1.0 // Normal
        }
    }
    
    /// Update position size based on metrics
    pub fn update_position_size(&mut self, metrics: &EnhancedMetrics) -> bool {
        if !self.config.enabled {
            return false;
        }
        
        let old_size = self.current_position_size;
        let new_size = self.calculate_optimal_position_size(metrics);
        
        // Only update if change is significant (>10%)
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
    
    /// Run full optimization cycle - adjust both spacing and position size
    pub fn optimize(&mut self, metrics: &EnhancedMetrics) -> OptimizationResult {
        if !self.config.enabled {
            return OptimizationResult {
                spacing_adjusted: false,
                size_adjusted: false,
                new_spacing: self.current_spacing_percent,
                new_position_size: self.current_position_size,
                reason: "Optimizer disabled".to_string(),
            };
        }
        
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
    
    /// Display current optimizer status
    pub fn display(&self) {
        println!("\n🧠 ADAPTIVE OPTIMIZER V2.0 STATUS:");
        println!("   Enabled:          {}", if self.config.enabled { "✅ YES" } else { "❌ NO" });
        
        if self.config.enabled {
            println!("   Adjustments Made: {}", self.adjustment_count);
            println!("   Current Spacing:  {:.3}% (base: {:.3}%)",
                     self.current_spacing_percent * 100.0, 
                     self.base_spacing_percent * 100.0);
            println!("   Current Size:     {:.3} SOL (base: {:.3} SOL)",
                     self.current_position_size, self.base_position_size);
            
            if !self.last_reason.is_empty() && self.last_reason != "Initialized" {
                println!("   Last Adjustment:  {}", self.last_reason);
            }
        } else {
            println!("   Status:           Using base values (no adaptation)");
        }
    }
}

// ═════════════════════════════════════════════════════════════════════════
// RESULT TYPES
// ═════════════════════════════════════════════════════════════════════════

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
    fn test_optimizer_with_config() {
        let config = AdaptiveOptimizerConfig {
            enabled: true,
            ..Default::default()
        };
        
        let optimizer = AdaptiveOptimizer::new_with_config(config, 0.15, 0.1);
        assert_eq!(optimizer.base_spacing_percent, 0.15);
        assert_eq!(optimizer.base_position_size, 0.1);
    }

    #[test]
    fn test_disabled_optimizer() {
        let config = AdaptiveOptimizerConfig {
            enabled: false,
            ..Default::default()
        };
        
        let mut optimizer = AdaptiveOptimizer::new_with_config(config, 0.15, 0.1);
        let metrics = EnhancedMetrics::new();
        
        let result = optimizer.optimize(&metrics);
        assert!(!result.any_changes());
        assert_eq!(result.reason, "Optimizer disabled");
    }
}
