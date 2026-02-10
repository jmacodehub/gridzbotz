//! ═══════════════════════════════════════════════════════════════════════════
//! 🛡️ SLIPPAGE GUARDIAN - Adaptive Slippage Protection
//! ═══════════════════════════════════════════════════════════════════════════
//!
//! **THE PROBLEM:**
//! - MEV bots profit from YOUR slippage tolerance
//! - High slippage = sandwich attack opportunity
//! - Fixed slippage doesn't adapt to market conditions
//!
//! **OUR SOLUTION:**
//! - Validate slippage BEFORE submitting transaction
//! - Reject trades exceeding safety threshold
//! - Adaptive tolerance based on volatility (optional)
//!
//! **CONSERVATIVE STRATEGY:**
//! - Max slippage: 0.5% (50 bps)
//! - Tighter in calm markets, looser in volatile
//! - Clear rejection messages for debugging
//!
//! ═══════════════════════════════════════════════════════════════════════════

use anyhow::{bail, Result};
use log::{debug, warn};
use serde::{Deserialize, Serialize};

// ═══════════════════════════════════════════════════════════════════════════
//  📊 SLIPPAGE CONFIGURATION
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageConfig {
    /// Enable slippage protection
    pub enabled: bool,
    
    /// Maximum allowed slippage in basis points (50 = 0.5%)
    pub max_slippage_bps: u16,
    
    /// Enable dynamic adjustment based on volatility
    pub dynamic_adjustment: bool,
    
    /// Multiplier for volatile markets (default: 1.2x)
    pub volatility_multiplier: f64,
}

impl Default for SlippageConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            max_slippage_bps: 50, // Conservative 0.5%
            dynamic_adjustment: true,
            volatility_multiplier: 1.2,
        }
    }
}

impl SlippageConfig {
    pub fn validate(&self) -> Result<()> {
        if self.max_slippage_bps > 1000 {
            bail!("max_slippage_bps too high: {} (max 1000 = 10%)", self.max_slippage_bps);
        }
        
        if self.volatility_multiplier < 1.0 || self.volatility_multiplier > 3.0 {
            bail!("volatility_multiplier must be 1.0-3.0, got {}", self.volatility_multiplier);
        }
        
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🎯 SLIPPAGE VALIDATION RESULT
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SlippageValidation {
    /// Is the slippage acceptable?
    pub is_acceptable: bool,
    
    /// Actual slippage in basis points
    pub slippage_bps: u16,
    
    /// Maximum allowed slippage in basis points
    pub max_slippage_bps: u16,
    
    /// Human-readable message
    pub message: String,
}

impl SlippageValidation {
    pub fn accepted(slippage_bps: u16, max_slippage_bps: u16) -> Self {
        Self {
            is_acceptable: true,
            slippage_bps,
            max_slippage_bps,
            message: format!("Slippage acceptable: {:.2}% <= {:.2}%", 
                slippage_bps as f64 / 100.0, 
                max_slippage_bps as f64 / 100.0
            ),
        }
    }
    
    pub fn rejected(slippage_bps: u16, max_slippage_bps: u16) -> Self {
        Self {
            is_acceptable: false,
            slippage_bps,
            max_slippage_bps,
            message: format!(
                "❌ Slippage too high: {:.2}% > {:.2}% (REJECTED)",
                slippage_bps as f64 / 100.0,
                max_slippage_bps as f64 / 100.0
            ),
        }
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// 🛡️ SLIPPAGE GUARDIAN
// ═══════════════════════════════════════════════════════════════════════════

pub struct SlippageGuardian {
    config: SlippageConfig,
}

impl SlippageGuardian {
    pub fn new(config: SlippageConfig) -> Self {
        Self { config }
    }
    
    /// Validate trade slippage before execution
    pub fn validate(
        &self,
        expected_price: f64,
        actual_price: f64,
    ) -> Result<SlippageValidation> {
        if !self.config.enabled {
            return Ok(SlippageValidation {
                is_acceptable: true,
                slippage_bps: 0,
                max_slippage_bps: 0,
                message: "Slippage protection disabled".to_string(),
            });
        }
        
        // Calculate slippage in basis points
        let slippage_pct = ((actual_price - expected_price).abs() / expected_price) * 100.0;
        let slippage_bps = (slippage_pct * 100.0).round() as u16;
        
        let max_allowed = self.config.max_slippage_bps;
        
        debug!(
            "💱 Slippage check: expected ${:.4}, actual ${:.4}, slippage {:.2}% ({}bps)",
            expected_price, actual_price, slippage_pct, slippage_bps
        );
        
        if slippage_bps <= max_allowed {
            Ok(SlippageValidation::accepted(slippage_bps, max_allowed))
        } else {
            warn!(
                "⚠️  Slippage too high: {:.2}% > {:.2}%",
                slippage_bps as f64 / 100.0,
                max_allowed as f64 / 100.0
            );
            Ok(SlippageValidation::rejected(slippage_bps, max_allowed))
        }
    }
    
    /// Validate with adaptive tolerance based on volatility
    pub fn validate_adaptive(
        &self,
        expected_price: f64,
        actual_price: f64,
        current_volatility_pct: f64,
    ) -> Result<SlippageValidation> {
        if !self.config.dynamic_adjustment {
            return self.validate(expected_price, actual_price);
        }
        
        // Adjust max slippage based on volatility
        let volatility_adjustment = if current_volatility_pct > 5.0 {
            self.config.volatility_multiplier
        } else {
            1.0
        };
        
        let adjusted_max = (self.config.max_slippage_bps as f64 * volatility_adjustment) as u16;
        let adjusted_max = adjusted_max.min(1000); // Never exceed 10%
        
        debug!(
            "🔄 Adaptive slippage: volatility {:.1}%, max adjusted to {}bps",
            current_volatility_pct, adjusted_max
        );
        
        // Temporarily adjust config and validate
        let mut temp_config = self.config.clone();
        temp_config.max_slippage_bps = adjusted_max;
        let temp_guardian = SlippageGuardian::new(temp_config);
        
        temp_guardian.validate(expected_price, actual_price)
    }
    
    /// Get current configuration
    pub fn config(&self) -> &SlippageConfig {
        &self.config
    }
}

// ═══════════════════════════════════════════════════════════════════════════
// ✅ TESTS
// ═══════════════════════════════════════════════════════════════════════════

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_config_validation() {
        let mut config = SlippageConfig::default();
        assert!(config.validate().is_ok());
        
        config.max_slippage_bps = 2000;
        assert!(config.validate().is_err());
        
        config.max_slippage_bps = 50;
        config.volatility_multiplier = 5.0;
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_slippage_acceptance() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // 0.3% slippage should be accepted (max is 0.5%)
        let result = guardian.validate(100.0, 100.3).unwrap();
        assert!(result.is_acceptable);
        assert_eq!(result.slippage_bps, 30); // 0.3% = 30bps
    }

    #[test]
    fn test_slippage_rejection() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // 1.0% slippage should be rejected (max is 0.5%)
        let result = guardian.validate(100.0, 101.0).unwrap();
        assert!(!result.is_acceptable);
        assert_eq!(result.slippage_bps, 100); // 1.0% = 100bps
    }

    #[test]
    fn test_adaptive_slippage_calm_market() {
        let config = SlippageConfig {
            enabled: true,
            max_slippage_bps: 50,
            dynamic_adjustment: true,
            volatility_multiplier: 1.5,
        };
        let guardian = SlippageGuardian::new(config);
        
        // Low volatility (3%) - normal max applies
        let result = guardian.validate_adaptive(100.0, 100.4, 3.0).unwrap();
        assert!(result.is_acceptable); // 0.4% < 0.5%
    }

    #[test]
    fn test_adaptive_slippage_volatile_market() {
        let config = SlippageConfig {
            enabled: true,
            max_slippage_bps: 50,
            dynamic_adjustment: true,
            volatility_multiplier: 1.5,
        };
        let guardian = SlippageGuardian::new(config);
        
        // High volatility (8%) - max becomes 0.75% (50bps * 1.5)
        let result = guardian.validate_adaptive(100.0, 100.7, 8.0).unwrap();
        assert!(result.is_acceptable); // 0.7% < 0.75%
    }

    #[test]
    fn test_disabled_protection() {
        let config = SlippageConfig {
            enabled: false,
            ..Default::default()
        };
        let guardian = SlippageGuardian::new(config);
        
        // Any slippage should be accepted when disabled
        let result = guardian.validate(100.0, 110.0).unwrap();
        assert!(result.is_acceptable);
    }

    #[test]
    fn test_slippage_calculation_symmetry() {
        let config = SlippageConfig::default();
        let guardian = SlippageGuardian::new(config);
        
        // Slippage should be same for price increase or decrease
        let result_up = guardian.validate(100.0, 100.5).unwrap();
        let result_down = guardian.validate(100.0, 99.5).unwrap();
        
        assert_eq!(result_up.slippage_bps, result_down.slippage_bps);
    }
}
