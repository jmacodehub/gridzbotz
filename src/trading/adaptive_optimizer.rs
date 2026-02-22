//! ═════════════════════════════════════════════════════════════════════════
//! 🧠 ADAPTIVE OPTIMIZER V1.0 - Self-Learning Grid Intelligence
//!
//! FEATURES:
//! ✅ Smart Grid Spacing - Auto-adjust based on performance
//! ✅ Dynamic Position Sizing - Scale orders based on efficiency
//! ✅ Real-time adaptation using EnhancedMetrics
//! ✅ Risk-aware adjustments
//! ✅ Win/Loss streak detection
//!
//! PHILOSOPHY:
//! "The bot that learns from its wins and losses is the bot that survives."
//!
//! February 9, 2026 - ADAPTIVE INTELLIGENCE ACTIVATED! 🔥
//! ═════════════════════════════════════════════════════════════════════════

use super::EnhancedMetrics;
use log::{info, debug, warn};

// ═════════════════════════════════════════════════════════════════════════
// CONSTANTS - Tuned for Production
// ═════════════════════════════════════════════════════════════════════════

// 🧠 Smart Grid Spacing Thresholds
const LOW_DRAWDOWN_THRESHOLD: f64 = 2.0;      // < 2% = doing great, tighten
const MODERATE_DRAWDOWN_THRESHOLD: f64 = 5.0; // 2-5% = normal, maintain
const HIGH_DRAWDOWN_THRESHOLD: f64 = 8.0;     // 5-8% = caution, widen
#[allow(dead_code)] // semantic label; else-branch catches > 8% via SPACING_EMERGENCY_MULTIPLIER
const EMERGENCY_DRAWDOWN_THRESHOLD: f64 = 12.0; // > 8% = emergency, max widen

// 🧠 Spacing Multipliers
const SPACING_TIGHTEN_MULTIPLIER: f64 = 0.80;   // 20% tighter when winning
const SPACING_NORMAL_MULTIPLIER: f64 = 1.00;    // Baseline
const SPACING_WIDEN_MULTIPLIER: f64 = 1.30;     // 30% wider when losing
const SPACING_EMERGENCY_MULTIPLIER: f64 = 1.80; // 80% wider in emergency

// 🧠 Limits
const MIN_SPACING_PERCENT: f64 = 0.01;  // 0.01% = absolute minimum (ultra tight)
const MAX_SPACING_PERCENT: f64 = 1.00;  // 1.0% = absolute maximum (very wide)

// ⚡ Dynamic Position Sizing Thresholds
const HIGH_EFFICIENCY_THRESHOLD: f64 = 0.70;  // > 70% efficiency = scale up
const LOW_EFFICIENCY_THRESHOLD: f64 = 0.30;   // < 30% efficiency = scale down

// ⚡ Position Size Multipliers
const SIZE_HIGH_EFFICIENCY_MULTIPLIER: f64 = 1.30;  // 30% bigger orders
const SIZE_NORMAL_MULTIPLIER: f64 = 1.00;           // Baseline
const SIZE_LOW_EFFICIENCY_MULTIPLIER: f64 = 0.70;   // 30% smaller orders

// ⚡ Win/Loss Streak Bonuses
const WIN_STREAK_BONUS_MAX: f64 = 1.50;   // Up to 50% bigger on win streaks
const LOSS_STREAK_PENALTY_MAX: f64 = 0.60; // Down to 40% smaller on loss streaks
const STREAK_THRESHOLD: usize = 3;         // 3+ wins/losses triggers adjustment

// ⚡ Limits
const MIN_POSITION_SIZE: f64 = 0.05;  // 0.05 SOL minimum
const MAX_POSITION_SIZE: f64 = 5.0;   // 5.0 SOL maximum

// ═════════════════════════════════════════════════════════════════════════
// ADAPTIVE OPTIMIZER - Main Struct
// ═════════════════════════════════════════════════════════════════════════

/// Adaptive optimizer that adjusts grid spacing and position sizing
/// based on real-time performance metrics
#[derive(Debug, Clone)]
pub struct AdaptiveOptimizer {
    /// Base grid spacing (from config)
    pub base_spacing_percent: f64,
    
    /// Base position size (from config)
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
    /// Create new optimizer with base settings from config
    pub fn new(base_spacing_percent: f64, base_position_size: f64) -> Self {
        info!("🧠 Initializing Adaptive Optimizer");
        info!("   Base Spacing: {:.3}%", base_spacing_percent);
        info!("   Base Position: {} SOL", base_position_size);
        
        Self {
            base_spacing_percent,
            base_position_size,
            current_spacing_percent: base_spacing_percent,
            current_position_size: base_position_size,
            adjustment_count: 0,
            last_reason: "Initialized".to_string(),
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // 🧠 SMART GRID SPACING
    // ═══════════════════════════════════════════════════════════════════
    
    /// Calculate optimal grid spacing based on current drawdown
    pub fn calculate_optimal_spacing(&self, metrics: &EnhancedMetrics) -> f64 {
        let drawdown = metrics.max_drawdown;
        
        // Determine multiplier based on drawdown
        let multiplier = if drawdown < LOW_DRAWDOWN_THRESHOLD {
            // Doing great! Tighten grid to catch more moves
            debug!("🎯 Low drawdown ({:.2}%) - tightening grid", drawdown);
            SPACING_TIGHTEN_MULTIPLIER
        } else if drawdown < MODERATE_DRAWDOWN_THRESHOLD {
            // Normal operation
            debug!("⚖️ Moderate drawdown ({:.2}%) - maintaining grid", drawdown);
            SPACING_NORMAL_MULTIPLIER
        } else if drawdown < HIGH_DRAWDOWN_THRESHOLD {
            // Caution - widen to reduce risk
            debug!("⚠️ High drawdown ({:.2}%) - widening grid", drawdown);
            SPACING_WIDEN_MULTIPLIER
        } else {
            // Emergency - max widen for capital preservation
            warn!("🚨 EMERGENCY drawdown ({:.2}%) - max widening!", drawdown);
            SPACING_EMERGENCY_MULTIPLIER
        };
        
        // Calculate new spacing
        let new_spacing = self.base_spacing_percent * multiplier;
        
        // Clamp to safe limits
        new_spacing.clamp(MIN_SPACING_PERCENT, MAX_SPACING_PERCENT)
    }
    
    /// Update grid spacing based on metrics
    pub fn update_spacing(&mut self, metrics: &EnhancedMetrics) -> bool {
        let old_spacing = self.current_spacing_percent;
        let new_spacing = self.calculate_optimal_spacing(metrics);
        
        // Only update if change is significant (>5%)
        let change_pct = ((new_spacing - old_spacing).abs() / old_spacing) * 100.0;
        
        if change_pct > 5.0 {
            self.current_spacing_percent = new_spacing;
            self.adjustment_count += 1;
            
            let drawdown = metrics.max_drawdown;
            self.last_reason = format!(
                "Drawdown {:.2}%: {:.3}% -> {:.3}%",
                drawdown, old_spacing, new_spacing
            );
            
            info!("🧠 SPACING ADJUSTED: {}", self.last_reason);
            true
        } else {
            debug!("Spacing change too small ({:.1}%), keeping current", change_pct);
            false
        }
    }
    
    // ═══════════════════════════════════════════════════════════════════
    // ⚡ DYNAMIC POSITION SIZING
    // ═══════════════════════════════════════════════════════════════════
    
    /// Calculate optimal position size based on grid efficiency
    pub fn calculate_optimal_position_size(&self, metrics: &EnhancedMetrics) -> f64 {
        let efficiency = metrics.grid_efficiency;
        
        // Base multiplier from efficiency
        let efficiency_multiplier = if efficiency > HIGH_EFFICIENCY_THRESHOLD {
            // High efficiency = scale up!
            debug!("📈 High efficiency ({:.1}%) - scaling up orders", efficiency * 100.0);
            SIZE_HIGH_EFFICIENCY_MULTIPLIER
        } else if efficiency < LOW_EFFICIENCY_THRESHOLD {
            // Low efficiency = scale down
            debug!("📉 Low efficiency ({:.1}%) - scaling down orders", efficiency * 100.0);
            SIZE_LOW_EFFICIENCY_MULTIPLIER
        } else {
            // Normal efficiency
            SIZE_NORMAL_MULTIPLIER
        };
        
        // Win/Loss streak adjustment
        let streak_multiplier = self.calculate_streak_multiplier(metrics);
        
        // Combine multipliers
        let total_multiplier = efficiency_multiplier * streak_multiplier;
        
        // Calculate new size
        let new_size = self.base_position_size * total_multiplier;
        
        // Clamp to safe limits
        new_size.clamp(MIN_POSITION_SIZE, MAX_POSITION_SIZE)
    }
    
    /// Calculate multiplier based on win/loss streaks
    fn calculate_streak_multiplier(&self, metrics: &EnhancedMetrics) -> f64 {
        let total_trades = metrics.profitable_trades + metrics.unprofitable_trades;
        
        if total_trades < STREAK_THRESHOLD {
            return 1.0; // Not enough data yet
        }
        
        // Simple streak detection: compare recent profitable vs unprofitable
        let win_rate = if total_trades > 0 {
            metrics.profitable_trades as f64 / total_trades as f64
        } else {
            0.5
        };
        
        if win_rate > 0.70 {
            // Strong win rate - bonus!
            let bonus = 1.0 + ((win_rate - 0.70) * 1.67); // Scale 0.70-1.0 to 1.0-1.5
            debug!("✨ Win streak detected ({:.0}%) - bonus {:.2}x", win_rate * 100.0, bonus);
            bonus.min(WIN_STREAK_BONUS_MAX)
        } else if win_rate < 0.40 {
            // Poor win rate - penalty
            let penalty = 0.6 + (win_rate * 1.0); // Scale 0.0-0.40 to 0.6-1.0
            debug!("🚫 Loss streak detected ({:.0}%) - penalty {:.2}x", win_rate * 100.0, penalty);
            penalty.max(LOSS_STREAK_PENALTY_MAX)
        } else {
            1.0 // Normal
        }
    }
    
    /// Update position size based on metrics
    pub fn update_position_size(&mut self, metrics: &EnhancedMetrics) -> bool {
        let old_size = self.current_position_size;
        let new_size = self.calculate_optimal_position_size(metrics);
        
        // Only update if change is significant (>10%)
        let change_pct = ((new_size - old_size).abs() / old_size) * 100.0;
        
        if change_pct > 10.0 {
            self.current_position_size = new_size;
            self.adjustment_count += 1;
            
            let efficiency = metrics.grid_efficiency;
            self.last_reason = format!(
                "Efficiency {:.1}%: {:.3} -> {:.3} SOL",
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
        debug!("🧠 Running optimization cycle #{}", self.adjustment_count + 1);
        
        let spacing_changed = self.update_spacing(metrics);
        let size_changed = self.update_position_size(metrics);
        
        if spacing_changed || size_changed {
            info!("✅ Optimization applied: spacing={:.3}%, size={:.3} SOL",
                  self.current_spacing_percent, self.current_position_size);
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
        println!("\n🧠 ADAPTIVE OPTIMIZER STATUS:");
        println!("   Adjustments Made:   {}", self.adjustment_count);
        println!("   Current Spacing:    {:.3}% (base: {:.3}%)",
                 self.current_spacing_percent, self.base_spacing_percent);
        println!("   Current Size:       {:.3} SOL (base: {:.3} SOL)",
                 self.current_position_size, self.base_position_size);
        
        if !self.last_reason.is_empty() && self.last_reason != "Initialized" {
            println!("   Last Adjustment:    {}", self.last_reason);
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
    fn test_optimizer_creation() {
        let optimizer = AdaptiveOptimizer::new(0.15, 0.1);
        assert_eq!(optimizer.base_spacing_percent, 0.15);
        assert_eq!(optimizer.base_position_size, 0.1);
        assert_eq!(optimizer.current_spacing_percent, 0.15);
        assert_eq!(optimizer.current_position_size, 0.1);
    }

    #[test]
    fn test_spacing_tightens_on_low_drawdown() {
        let optimizer = AdaptiveOptimizer::new(0.15, 0.1);
        let mut metrics = EnhancedMetrics::new();
        metrics.max_drawdown = 1.0; // Low drawdown
        
        let new_spacing = optimizer.calculate_optimal_spacing(&metrics);
        assert!(new_spacing < optimizer.base_spacing_percent);
    }

    #[test]
    fn test_spacing_widens_on_high_drawdown() {
        let optimizer = AdaptiveOptimizer::new(0.15, 0.1);
        let mut metrics = EnhancedMetrics::new();
        metrics.max_drawdown = 10.0; // High drawdown
        
        let new_spacing = optimizer.calculate_optimal_spacing(&metrics);
        assert!(new_spacing > optimizer.base_spacing_percent);
    }

    #[test]
    fn test_position_size_scales_with_efficiency() {
        let optimizer = AdaptiveOptimizer::new(0.15, 0.1);
        let mut metrics = EnhancedMetrics::new();
        
        // High efficiency
        metrics.grid_efficiency = 0.80;
        let high_eff_size = optimizer.calculate_optimal_position_size(&metrics);
        
        // Low efficiency
        metrics.grid_efficiency = 0.20;
        let low_eff_size = optimizer.calculate_optimal_position_size(&metrics);
        
        assert!(high_eff_size > low_eff_size);
    }

    #[test]
    fn test_limits_enforced() {
        let optimizer = AdaptiveOptimizer::new(0.01, 0.01);
        let mut metrics = EnhancedMetrics::new();
        metrics.max_drawdown = 50.0; // Extreme drawdown
        
        let spacing = optimizer.calculate_optimal_spacing(&metrics);
        assert!(spacing <= MAX_SPACING_PERCENT);
        assert!(spacing >= MIN_SPACING_PERCENT);
        
        let size = optimizer.calculate_optimal_position_size(&metrics);
        assert!(size <= MAX_POSITION_SIZE);
        assert!(size >= MIN_POSITION_SIZE);
    }
}
