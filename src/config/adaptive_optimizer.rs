//! ═══════════════════════════════════════════════════════════════════════════
//! 🤖 ADAPTIVE OPTIMIZER CONFIGURATION
//! ═══════════════════════════════════════════════════════════════════════════

use serde::{Deserialize, Serialize};
use anyhow::{Result, bail};
use log::warn;

/// Adaptive Optimizer Configuration
/// 
/// Controls the AI-powered grid spacing and position sizing optimization.
/// All thresholds and multipliers are configurable for fine-tuning.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct AdaptiveOptimizerConfig {
    /// Enable adaptive optimizer
    #[serde(default)]
    pub enabled: bool,
    
    /// Run optimization every N cycles
    #[serde(default = "default_optimization_interval")]
    pub optimization_interval_cycles: u32,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Spacing AI Thresholds
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Drawdown threshold for tightening grid (< this = tighten)
    #[serde(default = "default_low_drawdown")]
    pub low_drawdown_threshold: f64,
    
    /// Drawdown threshold for maintaining grid
    #[serde(default = "default_moderate_drawdown")]
    pub moderate_drawdown_threshold: f64,
    
    /// Drawdown threshold for widening grid
    #[serde(default = "default_high_drawdown")]
    pub high_drawdown_threshold: f64,
    
    /// Drawdown threshold for emergency widening
    #[serde(default = "default_emergency_drawdown")]
    pub emergency_drawdown_threshold: f64,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Spacing AI Multipliers
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Multiplier when tightening (e.g., 0.80 = 20% tighter)
    #[serde(default = "default_tighten_multiplier")]
    pub spacing_tighten_multiplier: f64,
    
    /// Multiplier for normal operation
    #[serde(default = "default_normal_multiplier")]
    pub spacing_normal_multiplier: f64,
    
    /// Multiplier when widening
    #[serde(default = "default_widen_multiplier")]
    pub spacing_widen_multiplier: f64,
    
    /// Multiplier for emergency widening
    #[serde(default = "default_emergency_multiplier")]
    pub spacing_emergency_multiplier: f64,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Position Sizing AI
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Grid efficiency threshold for scaling up (> this = bigger orders)
    #[serde(default = "default_high_efficiency")]
    pub high_efficiency_threshold: f64,
    
    /// Grid efficiency threshold for scaling down (< this = smaller orders)
    #[serde(default = "default_low_efficiency")]
    pub low_efficiency_threshold: f64,
    
    /// Multiplier for high efficiency
    #[serde(default = "default_high_efficiency_multiplier")]
    pub size_high_efficiency_multiplier: f64,
    
    /// Multiplier for low efficiency
    #[serde(default = "default_low_efficiency_multiplier")]
    pub size_low_efficiency_multiplier: f64,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Win/Loss Streak Bonuses
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Maximum bonus multiplier for win streaks
    #[serde(default = "default_win_bonus")]
    pub win_streak_bonus_max: f64,
    
    /// Maximum penalty multiplier for loss streaks
    #[serde(default = "default_loss_penalty")]
    pub loss_streak_penalty_max: f64,
    
    /// Number of consecutive wins/losses to trigger adjustment
    #[serde(default = "default_streak_threshold")]
    pub streak_threshold: usize,
    
    // ─────────────────────────────────────────────────────────────────────────
    // Safety Limits
    // ─────────────────────────────────────────────────────────────────────────
    
    /// Absolute minimum grid spacing %
    #[serde(default = "default_min_spacing")]
    pub min_spacing_absolute: f64,
    
    /// Absolute maximum grid spacing %
    #[serde(default = "default_max_spacing")]
    pub max_spacing_absolute: f64,
    
    /// Absolute minimum position size (SOL)
    #[serde(default = "default_min_position")]
    pub min_position_absolute: f64,
    
    /// Absolute maximum position size (SOL)
    #[serde(default = "default_max_position")]
    pub max_position_absolute: f64,
}

impl Default for AdaptiveOptimizerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            optimization_interval_cycles: default_optimization_interval(),
            
            // Spacing thresholds
            low_drawdown_threshold: default_low_drawdown(),
            moderate_drawdown_threshold: default_moderate_drawdown(),
            high_drawdown_threshold: default_high_drawdown(),
            emergency_drawdown_threshold: default_emergency_drawdown(),
            
            // Spacing multipliers
            spacing_tighten_multiplier: default_tighten_multiplier(),
            spacing_normal_multiplier: default_normal_multiplier(),
            spacing_widen_multiplier: default_widen_multiplier(),
            spacing_emergency_multiplier: default_emergency_multiplier(),
            
            // Efficiency thresholds
            high_efficiency_threshold: default_high_efficiency(),
            low_efficiency_threshold: default_low_efficiency(),
            
            // Efficiency multipliers
            size_high_efficiency_multiplier: default_high_efficiency_multiplier(),
            size_low_efficiency_multiplier: default_low_efficiency_multiplier(),
            
            // Streak bonuses
            win_streak_bonus_max: default_win_bonus(),
            loss_streak_penalty_max: default_loss_penalty(),
            streak_threshold: default_streak_threshold(),
            
            // Safety limits
            min_spacing_absolute: default_min_spacing(),
            max_spacing_absolute: default_max_spacing(),
            min_position_absolute: default_min_position(),
            max_position_absolute: default_max_position(),
        }
    }
}

impl AdaptiveOptimizerConfig {
    pub fn validate(&self) -> Result<()> {
        // Validate thresholds are in order
        if self.low_drawdown_threshold >= self.moderate_drawdown_threshold {
            bail!("low_drawdown_threshold must be < moderate_drawdown_threshold");
        }
        if self.moderate_drawdown_threshold >= self.high_drawdown_threshold {
            bail!("moderate_drawdown_threshold must be < high_drawdown_threshold");
        }
        if self.high_drawdown_threshold >= self.emergency_drawdown_threshold {
            bail!("high_drawdown_threshold must be < emergency_drawdown_threshold");
        }
        
        // Validate multipliers are reasonable
        if self.spacing_tighten_multiplier >= 1.0 {
            bail!("spacing_tighten_multiplier must be < 1.0 (tightening)");
        }
        if self.spacing_widen_multiplier <= 1.0 {
            bail!("spacing_widen_multiplier must be > 1.0 (widening)");
        }
        if self.spacing_emergency_multiplier <= self.spacing_widen_multiplier {
            bail!("spacing_emergency_multiplier must be > spacing_widen_multiplier");
        }
        
        // Validate efficiency thresholds
        if self.low_efficiency_threshold >= self.high_efficiency_threshold {
            bail!("low_efficiency_threshold must be < high_efficiency_threshold");
        }
        
        // Validate limits
        if self.min_spacing_absolute >= self.max_spacing_absolute {
            bail!("min_spacing_absolute must be < max_spacing_absolute");
        }
        if self.min_position_absolute >= self.max_position_absolute {
            bail!("min_position_absolute must be < max_position_absolute");
        }
        
        // Warnings for extreme values
        if self.min_spacing_absolute < 0.01 {
            warn!("⚠️ Very tight min_spacing ({:.3}%) - may not profit after fees",
                  self.min_spacing_absolute);
        }
        if self.max_spacing_absolute > 1.0 {
            warn!("⚠️ Very wide max_spacing ({:.2}%) - trades may be infrequent",
                  self.max_spacing_absolute);
        }
        
        Ok(())
    }
}

// Default value functions
fn default_optimization_interval() -> u32 { 10 }
fn default_low_drawdown() -> f64 { 2.0 }
fn default_moderate_drawdown() -> f64 { 5.0 }
fn default_high_drawdown() -> f64 { 8.0 }
fn default_emergency_drawdown() -> f64 { 12.0 }
fn default_tighten_multiplier() -> f64 { 0.80 }
fn default_normal_multiplier() -> f64 { 1.00 }
fn default_widen_multiplier() -> f64 { 1.30 }
fn default_emergency_multiplier() -> f64 { 1.80 }
fn default_high_efficiency() -> f64 { 0.70 }
fn default_low_efficiency() -> f64 { 0.30 }
fn default_high_efficiency_multiplier() -> f64 { 1.30 }
fn default_low_efficiency_multiplier() -> f64 { 0.70 }
fn default_win_bonus() -> f64 { 1.50 }
fn default_loss_penalty() -> f64 { 0.60 }
fn default_streak_threshold() -> usize { 3 }
fn default_min_spacing() -> f64 { 0.01 }
fn default_max_spacing() -> f64 { 1.00 }
fn default_min_position() -> f64 { 0.05 }
fn default_max_position() -> f64 { 5.0 }
